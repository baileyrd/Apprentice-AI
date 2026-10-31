import csv,sys,math,collections
rows=list(csv.DictReader(open(sys.argv[1])))
arms=['R','RU','RS','RUS']
per=collections.defaultdict(lambda: collections.defaultdict(lambda:[0,0,0]))
for r in rows:
    w=int(r['wrong']); k=per[r['arm']][int(r['seed'])]
    k[0]+=w
    if r['place']=='stranger': k[1]+=w
    else: k[2]+=w
seeds=sorted(per['R'])
def iv(d):
    n=len(d); m=sum(d)/n; sd=math.sqrt(sum((x-m)**2 for x in d)/(n-1)); se=sd/math.sqrt(n); return m,m-1.645*se,m+1.645*se
def geo(b,t):
    l=[math.log((y+1)/(x+1)) for x,y in zip(b,t)]; m,lo,hi=iv(l); return [math.exp(v)-1 for v in (m,lo,hi)]
print('n seeds',len(seeds))
for i,lab in enumerate(['total','stranger->home','home->stranger']):
    print(lab, {a: round(sum(per[a][s][i] for s in seeds)/len(seeds),3) for a in arms})
    for a in arms[1:]:
        b=[per['R'][s][i] for s in seeds]; t=[per[a][s][i] for s in seeds]
        m,lo,hi=iv([y-x for x,y in zip(b,t)]); g=geo(b,t)
        print(f'  {a}-R raw {m:+.3f} [{lo:+.3f},{hi:+.3f}]  pct {100*g[0]:+.0f}% [{100*g[1]:+.0f}%,{100*g[2]:+.0f}%]')
