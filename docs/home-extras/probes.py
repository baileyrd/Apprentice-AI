import csv,collections,statistics as st
rows=list(csv.DictReader(open('docs/home-extras/probes.csv')))
S=[r for r in rows if r['place']=='stranger']
print('arm variant | stranger wrong/180 | visited | pat_n | pat_err | map_n | map_err | home_fit')
for a in ['R','RU','RS','RUS']:
  for v in ['base','nofit','flip']:
    rs=[r for r in S if r['arm']==a and r['variant']==v]
    m=lambda k: st.mean(float(r[k]) for r in rs)
    print(f"{a:4}{v:6} {sum(int(r['wrong']) for r in rs):3} vis {m('visited'):.1f} pat_n {m('pat_n'):.0f} pat_err {m('pat_err'):.3f} map_n {m('map_n'):.0f} map_err {m('map_err'):.3f} fit {m('home_fit'):.3f}")
# home/changed wrong
print('home wrong', {(a,v):sum(int(r['wrong']) for r in rows if r['place']!='stranger' and r['arm']==a and r['variant']==v) for a in ['R','RS'] for v in ['base','nofit','flip']})
# wrong vs right strangers, base: visited and fit
for a in ['R','RS']:
  for w in '01':
    rs=[r for r in S if r['arm']==a and r['variant']=='base' and r['wrong']==w]
    print(a,'wrong' if w=='1' else 'right', len(rs), 'visited med',st.median(int(r['visited']) for r in rs),'map_n med',st.median(int(r['map_n']) for r in rs),'map_err med %.3f'%st.median(float(r['map_err']) for r in rs),'fit med %.3f'%st.median(float(r['home_fit']) for r in rs))
