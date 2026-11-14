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

use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

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
            PlugError::BadReply { line, why } => write!(f, "bad reply {line:?}: {why}"),
        }
    }
}

impl std::error::Error for PlugError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlugError::Launch(e) | PlugError::Write(e) | PlugError::Read(e) => Some(e),
            PlugError::Closed | PlugError::BadReply { .. } => None,
        }
    }
}

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
pub struct Plugged {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    /// The last place it named; `None` until it names one.
    place: Option<usize>,
}

impl Plugged {
    /// Launches `cmd` (program, then arguments), with `seed` as its last
    /// argument.
    pub fn launch(cmd: &[String], seed: u64) -> Result<Self, PlugError> {
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
        let missing = || PlugError::Launch(io::Error::other("no pipe to the agent"));
        let stdin = child.stdin.take().ok_or_else(missing)?;
        let stdout = BufReader::new(child.stdout.take().ok_or_else(missing)?);
        Ok(Self {
            child,
            stdin,
            stdout,
            place: None,
        })
    }

    fn send(&mut self, msg: &ToAgent) -> Result<(), PlugError> {
        let mut line = serde_json::to_vec(msg).map_err(|e| PlugError::Write(e.into()))?;
        line.push(b'\n');
        self.stdin.write_all(&line).map_err(PlugError::Write)?;
        self.stdin.flush().map_err(PlugError::Write)
    }

    fn reply(&mut self) -> Result<Reply, PlugError> {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).map_err(PlugError::Read)?;
        if read == 0 {
            return Err(PlugError::Closed);
        }
        serde_json::from_str(line.trim_end()).map_err(|e| PlugError::BadReply {
            why: e.to_string(),
            line,
        })
    }
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
    /// The life is over: stop the process. Errors are ignored on purpose:
    /// it may already have exited, and a drop has no one to report to.
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

    /// A shell agent running `script`.
    fn shell(script: &str) -> Plugged {
        let cmd = ["sh".to_string(), "-c".to_string(), script.to_string()];
        Plugged::launch(&cmd, 1).expect("sh launches")
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
    fn a_missing_program_fails_to_launch() {
        let cmd = ["/no/such/agent".to_string()];
        let e = Plugged::launch(&cmd, 1).err().expect("fails");
        assert!(matches!(e, PlugError::Launch(_)), "{e}");
        assert!(matches!(Plugged::launch(&[], 1), Err(PlugError::Launch(_))));
    }

    #[test]
    fn agent_seed_differs_from_the_world_seed() {
        assert!((0..100).all(|s| agent_seed(s) != s));
        assert_eq!(agent_seed(5), agent_seed(5));
    }
}
