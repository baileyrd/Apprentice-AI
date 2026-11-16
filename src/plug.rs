//! An agent in another process, spoken to in JSON lines (docs/port).
//!
//! The world writes one message per line to the agent's stdin:
//!
//! ```text
//! {"heard": {"at": {"x": 1, "y": -2}, "voice": 0, "word": 4}}
//! {"sense": {"view": ["Wall", "Empty", ...], "energy": 47, "offset": {"x": 3, "y": 0}}}
//! {"felt": {"event": "Ate", "energy": 71, "slept": false}}
//! ```
//!
//! and reads one line from its stdout after each `sense`:
//!
//! ```text
//! {"act": "North"}             North, East, South, West, Eat or Rest
//! {"act": "Eat", "place": 2}   place: optional, its own name for the world
//! ```
//!
//! Nothing else is sent: no world id, seed, size, step count or "new world".
//! The agent gets its seed once, as its last argument, and must be
//! deterministic given it. Its stderr passes through.
//!
//! Every write and every reply has a deadline, and a reply is at most
//! [`MAX_LINE`] bytes: a stuck or babbling agent fails its life, it does not
//! hang the run. Only the launched process is stopped when the life ends, so
//! an agent should `exec` its program rather than leave children holding the
//! pipes.

use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::body::{Action, Observation, Said};
use crate::episode::Port;
use crate::rng::Rng;
use crate::sim::Outcome;
use crate::world::{Dir, Pos};

/// What went wrong talking to the agent's process.
#[derive(Debug)]
pub enum PlugError {
    /// The command could not be started.
    Launch(io::Error),
    Write(io::Error),
    Read(io::Error),
    /// The agent closed its stdout instead of replying.
    Closed,
    /// The agent neither took a message nor replied within the deadline.
    Timeout(Duration),
    /// A reply longer than [`MAX_LINE`] bytes.
    TooLong,
    /// A reply that is not `{"act": ...}` with a known action.
    BadReply {
        line: String,
        why: String,
    },
}

impl fmt::Display for PlugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlugError::Launch(e) => write!(f, "could not launch the agent: {e}"),
            PlugError::Write(e) => write!(f, "could not write to the agent: {e}"),
            PlugError::Read(e) => write!(f, "could not read from the agent: {e}"),
            PlugError::Closed => write!(f, "the agent closed its output instead of replying"),
            PlugError::Timeout(d) => write!(f, "the agent did not answer within {d:?}"),
            PlugError::TooLong => write!(f, "a reply longer than {MAX_LINE} bytes"),
            PlugError::BadReply { line, why } => write!(f, "bad reply {line:?}: {why}"),
        }
    }
}

impl std::error::Error for PlugError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlugError::Launch(e) | PlugError::Write(e) | PlugError::Read(e) => Some(e),
            PlugError::Closed
            | PlugError::Timeout(_)
            | PlugError::TooLong
            | PlugError::BadReply { .. } => None,
        }
    }
}

/// The longest reply line, newline included.
pub const MAX_LINE: usize = 64 * 1024;

/// How long the world waits on an agent by default, per write or reply.
pub const DEADLINE: Duration = Duration::from_secs(10);

/// The seed an agent is launched with for a world built from `world_seed`:
/// derived from it, never equal to it.
pub fn agent_seed(world_seed: u64) -> u64 {
    Rng::new(world_seed ^ 0xA9E7_5EED).next_u64()
}

/// A message to the agent.
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum ToAgent<'a> {
    Heard { at: Pos, voice: u8, word: u8 },
    Sense(&'a Observation),
    Felt(&'a Outcome),
}

/// The agent's answer to `sense`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    act: Act,
    #[serde(default)]
    place: Option<usize>,
}

/// An action, as named on the wire.
#[derive(Deserialize)]
enum Act {
    North,
    East,
    South,
    West,
    Eat,
    Rest,
}

impl From<Act> for Action {
    fn from(act: Act) -> Self {
        match act {
            Act::North => Action::Move(Dir::North),
            Act::East => Action::Move(Dir::East),
            Act::South => Action::Move(Dir::South),
            Act::West => Action::Move(Dir::West),
            Act::Eat => Action::Eat,
            Act::Rest => Action::Rest,
        }
    }
}

/// An agent in another process. A [`Port`], never an `Agent`: talking to
/// it can fail.
///
/// Its pipes are worked by two threads, so the world can stop waiting:
/// one writes the lines it is given, the other reads bounded reply lines.
pub struct Plugged {
    child: Child,
    to_agent: Sender<Vec<u8>>,
    written: Receiver<io::Result<()>>,
    replies: Receiver<Result<String, PlugError>>,
    deadline: Duration,
    /// The last place it named; `None` until it names one.
    place: Option<usize>,
}

impl Plugged {
    /// Launches `cmd` (program, then arguments), with `seed` as its last
    /// argument, waiting at most `deadline` on each write or reply.
    pub fn launch(cmd: &[String], seed: u64, deadline: Duration) -> Result<Self, PlugError> {
        let (program, args) = cmd.split_first().ok_or_else(|| {
            PlugError::Launch(io::Error::new(io::ErrorKind::InvalidInput, "empty command"))
        })?;
        let mut child = Command::new(program)
            .args(args)
            .arg(seed.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(PlugError::Launch)?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PlugError::Launch(io::Error::other("no pipe to the agent")));
        };
        let (to_agent, lines) = mpsc::channel();
        let (acks, written) = mpsc::channel();
        let (replied, replies) = mpsc::channel();
        thread::spawn(move || write_lines(stdin, lines, acks));
        thread::spawn(move || read_lines(BufReader::new(stdout), replied));
        Ok(Self {
            child,
            to_agent,
            written,
            replies,
            deadline,
            place: None,
        })
    }

    fn send(&mut self, msg: &ToAgent) -> Result<(), PlugError> {
        let mut line = serde_json::to_vec(msg).map_err(|e| PlugError::Write(e.into()))?;
        line.push(b'\n');
        let gone = || PlugError::Write(io::ErrorKind::BrokenPipe.into());
        self.to_agent.send(line).map_err(|_| gone())?;
        match self.written.recv_timeout(self.deadline) {
            Ok(done) => done.map_err(PlugError::Write),
            Err(RecvTimeoutError::Timeout) => Err(PlugError::Timeout(self.deadline)),
            Err(RecvTimeoutError::Disconnected) => Err(gone()),
        }
    }

    fn reply(&mut self) -> Result<Reply, PlugError> {
        let line = match self.replies.recv_timeout(self.deadline) {
            Ok(line) => line?,
            Err(RecvTimeoutError::Timeout) => return Err(PlugError::Timeout(self.deadline)),
            Err(RecvTimeoutError::Disconnected) => return Err(PlugError::Closed),
        };
        serde_json::from_str(line.trim_end()).map_err(|e| PlugError::BadReply {
            why: e.to_string(),
            line,
        })
    }
}

/// Writes each line to the agent and acknowledges it, until a write fails
/// or the [`Plugged`] is gone.
fn write_lines(mut stdin: ChildStdin, lines: Receiver<Vec<u8>>, acks: Sender<io::Result<()>>) {
    for line in lines {
        let done = stdin.write_all(&line).and_then(|()| stdin.flush());
        let failed = done.is_err();
        if acks.send(done).is_err() || failed {
            return;
        }
    }
}

/// Reads reply lines until the first error, or until the [`Plugged`] is gone.
fn read_lines(mut stdout: impl BufRead, replies: Sender<Result<String, PlugError>>) {
    loop {
        let line = read_line(&mut stdout);
        let failed = line.is_err();
        if replies.send(line).is_err() || failed {
            return;
        }
    }
}

/// One line of at most [`MAX_LINE`] bytes. The last line may lack its newline.
fn read_line(from: &mut impl BufRead) -> Result<String, PlugError> {
    let mut line = Vec::new();
    let limit = MAX_LINE as u64 + 1;
    let read = from
        .take(limit)
        .read_until(b'\n', &mut line)
        .map_err(PlugError::Read)?;
    if read == 0 {
        return Err(PlugError::Closed);
    }
    if line.len() > MAX_LINE {
        return Err(PlugError::TooLong);
    }
    String::from_utf8(line).map_err(|e| PlugError::BadReply {
        why: e.to_string(),
        line: String::from_utf8_lossy(e.as_bytes()).into_owned(),
    })
}

impl Port for Plugged {
    type Error = PlugError;

    fn act(&mut self, obs: &Observation) -> Result<Action, PlugError> {
        self.send(&ToAgent::Sense(obs))?;
        let reply = self.reply()?;
        if reply.place.is_some() {
            self.place = reply.place;
        }
        Ok(reply.act.into())
    }

    fn feel(&mut self, outcome: &Outcome) -> Result<(), PlugError> {
        self.send(&ToAgent::Felt(outcome))
    }

    fn hear(&mut self, at: Pos, said: Said) -> Result<(), PlugError> {
        self.send(&ToAgent::Heard {
            at,
            voice: said.voice.0,
            word: said.word.0,
        })
    }

    fn place(&self) -> Option<usize> {
        self.place
    }
}

impl Drop for Plugged {
    /// The life is over: stop and reap the process, which ends both pipe
    /// threads. Errors are ignored on purpose: it may already have exited,
    /// and a drop has no one to report to.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::episode::{Phase, run};
    use crate::sim::{Config, Sim};

    fn sim() -> Sim {
        Sim::new(Config::default(), 7).expect("default layout has room")
    }

    /// A shell agent running `script`, waited on at most `deadline`.
    fn shell_within(script: &str, deadline: Duration) -> Plugged {
        let cmd = ["sh".to_string(), "-c".to_string(), script.to_string()];
        Plugged::launch(&cmd, 1, deadline).expect("sh launches")
    }

    fn shell(script: &str) -> Plugged {
        shell_within(script, Duration::from_secs(5))
    }

    /// Replies Rest, naming place 7, to every `sense`.
    const RESTER: &str =
        r#"while read l; do case "$l" in *sense*) echo '{"act":"Rest","place":7}';; esac; done"#;

    #[test]
    fn a_well_behaved_agent_lives() {
        let (mut world, mut agent) = (sim(), shell(RESTER));
        run(&mut world, &mut agent, &mut [], 50, &mut ()).expect("lives");
        assert_eq!(world.t(), 50);
        assert_eq!(agent.place(), Some(7));
    }

    /// Fails at the first `act`, with the world unmoved.
    fn fails_to_act(script: &str) -> PlugError {
        let (mut world, mut agent) = (sim(), shell(script));
        let e = run(&mut world, &mut agent, &mut [], 10, &mut ()).expect_err("fails");
        assert_eq!((e.t, e.during), (0, Phase::Act));
        assert_eq!(world.t(), 0);
        e.cause
    }

    #[test]
    fn a_reply_that_is_not_json_fails() {
        let e = fails_to_act("read l; echo nope; sleep 5");
        assert!(matches!(e, PlugError::BadReply { .. }), "{e}");
    }

    #[test]
    fn an_unknown_action_fails() {
        let e = fails_to_act(r#"read l; echo '{"act":"Fly"}'; sleep 5"#);
        assert!(matches!(e, PlugError::BadReply { .. }), "{e}");
    }

    #[test]
    fn a_reply_without_an_action_fails() {
        let e = fails_to_act(r#"read l; echo '{"place":1}'; sleep 5"#);
        assert!(matches!(e, PlugError::BadReply { .. }), "{e}");
    }

    #[test]
    fn an_agent_that_quits_before_replying_fails() {
        let e = fails_to_act("exit 0");
        assert!(matches!(e, PlugError::Closed | PlugError::Write(_)), "{e}");
    }

    #[test]
    fn an_agent_that_stops_listening_fails_to_feel() {
        // Closes its stdin, then replies, so the next write has no reader.
        let script = r#"read l; exec 0<&-; echo '{"act":"Rest"}'; sleep 5"#;
        let (mut world, mut agent) = (sim(), shell(script));
        let e = run(&mut world, &mut agent, &mut [], 10, &mut ()).expect_err("fails");
        assert_eq!((e.t, e.during), (1, Phase::Feel));
        assert!(matches!(e.cause, PlugError::Write(_)), "{}", e.cause);
    }

    #[test]
    fn a_silent_agent_times_out_with_the_world_unmoved() {
        let deadline = Duration::from_millis(200);
        let (mut world, mut agent) = (sim(), shell_within("read l; exec sleep 30", deadline));
        let begun = std::time::Instant::now();
        let e = run(&mut world, &mut agent, &mut [], 10, &mut ()).expect_err("fails");
        assert!(begun.elapsed() < Duration::from_secs(5), "bounded wait");
        assert_eq!((e.t, e.during), (0, Phase::Act));
        assert_eq!(world.t(), 0);
        assert!(
            matches!(e.cause, PlugError::Timeout(d) if d == deadline),
            "{}",
            e.cause
        );
    }

    #[test]
    fn a_reply_without_end_fails_as_too_long() {
        let e = fails_to_act("read l; exec head -c 100000 /dev/zero");
        assert!(matches!(e, PlugError::TooLong), "{e}");
    }

    #[test]
    fn a_missing_program_fails_to_launch() {
        let cmd = ["/no/such/agent".to_string()];
        let e = Plugged::launch(&cmd, 1, DEADLINE).err().expect("fails");
        assert!(matches!(e, PlugError::Launch(_)), "{e}");
        let empty = Plugged::launch(&[], 1, DEADLINE);
        assert!(matches!(empty, Err(PlugError::Launch(_))));
    }

    #[test]
    fn agent_seed_differs_from_the_world_seed() {
        assert!((0..100).all(|s| agent_seed(s) != s));
        assert_eq!(agent_seed(5), agent_seed(5));
    }
}

#[cfg(test)]
mod line_tests {
    use super::*;
    use std::io::Cursor;

    fn lines(bytes: &[u8]) -> Vec<Result<String, PlugError>> {
        let mut from = Cursor::new(bytes.to_vec());
        let mut got = vec![read_line(&mut from)];
        while got.last().is_some_and(Result::is_ok) {
            got.push(read_line(&mut from));
        }
        got
    }

    #[test]
    fn lines_are_read_one_by_one_then_closed() {
        let got = lines(b"a\nb");
        assert_eq!(got[0].as_deref().ok(), Some("a\n"));
        assert_eq!(
            got[1].as_deref().ok(),
            Some("b"),
            "last line needs no newline"
        );
        assert!(matches!(got[2], Err(PlugError::Closed)));
    }

    #[test]
    fn a_line_of_exactly_the_limit_is_read() {
        let mut line = vec![b'x'; MAX_LINE - 1];
        line.push(b'\n');
        assert_eq!(
            lines(&line)[0].as_ref().map(String::len).ok(),
            Some(MAX_LINE)
        );
    }

    #[test]
    fn a_line_over_the_limit_is_too_long() {
        let mut line = vec![b'x'; MAX_LINE];
        line.push(b'\n');
        assert!(matches!(lines(&line)[0], Err(PlugError::TooLong)));
        assert!(matches!(
            lines(&[b'x'; MAX_LINE + 1])[0],
            Err(PlugError::TooLong)
        ));
    }

    #[test]
    fn a_line_that_is_not_utf8_is_a_bad_reply() {
        assert!(matches!(
            lines(b"\xff\n")[0],
            Err(PlugError::BadReply { .. })
        ));
    }
}
