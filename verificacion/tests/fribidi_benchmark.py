import ctypes,json,sys,time
from pathlib import Path
lib=ctypes.CDLL(sys.argv[1])
U=ctypes.c_uint32;I=ctypes.c_int;L=ctypes.c_byte
lib.fribidi_get_bidi_type.argtypes=[U];lib.fribidi_get_bidi_type.restype=U
lib.fribidi_get_bidi_types.argtypes=[ctypes.POINTER(U),I,ctypes.POINTER(U)]
lib.fribidi_get_bracket_types.argtypes=[ctypes.POINTER(U),I,ctypes.POINTER(U),ctypes.POINTER(U)]
lib.fribidi_get_par_embedding_levels_ex.argtypes=[ctypes.POINTER(U),ctypes.POINTER(U),I,ctypes.POINTER(U),ctypes.POINTER(L)]
lib.fribidi_get_par_embedding_levels_ex.restype=L
rows=[]
for name,text in [('normal','Texto español con puntuación. '*100),('mixed','Spanish (العربية) עברית 123. '*100),('isolates','\u2067a\u2069'*2000),('brackets',('\u2067('*100+'abc'+')\u2069'*100)*5)]:
 n=len(text);chars=(U*n)(*map(ord,text));types=(U*n)();brackets=(U*n)();levels=(L*n)()
 lib.fribidi_get_bidi_types(chars,n,types);lib.fribidi_get_bracket_types(chars,n,types,brackets)
 samples=[]
 for _ in range(5):
  direction=U(lib.fribidi_get_bidi_type(ord('A')));start=time.perf_counter();result=lib.fribidi_get_par_embedding_levels_ex(types,brackets,n,ctypes.byref(direction),levels);samples.append(time.perf_counter()-start)
  assert result>0,(name,result)
 rows.append(dict(case=name,characters=n,median_ms=sorted(samples)[2]*1000,levels_sha256=__import__('hashlib').sha256(bytes(levels)).hexdigest()))
print(json.dumps(rows))
