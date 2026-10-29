"""Package pinned corresponding sources, excluding player/media/config binaries.

Run after cargo vendor and collect-engine-sources.py. ZIPs are independent.
"""
from __future__ import annotations
import argparse
import hashlib
import io
import json
import re
import subprocess
import tarfile
import zipfile
from pathlib import Path


def digest(path: Path) -> str:
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def add_tree(archive: zipfile.ZipFile, root: Path, prefix: str) -> None:
    for path in sorted(root.rglob('*')):
        if not path.is_file() or any(p in ('.git', '__pycache__', 'target') for p in path.relative_to(root).parts):
            continue
        if not path.resolve().is_relative_to(root.resolve()):
            raise ValueError(f'Source link outside tree: {path}')
        archive.write(path, prefix + '/' + path.relative_to(root).as_posix())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pinned', type=Path, required=True)
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--git', type=Path, required=True)
    parser.add_argument('--reuse-dependencies', type=Path)
    parser.add_argument('--reuse-sha256')
    args = parser.parse_args()
    app = Path(__file__).resolve().parents[1]
    patch = (app/'native/mpv-subtitle-stereo.patch').read_bytes()
    actual_patch = subprocess.check_output([str(args.git), '-C', str((args.pinned/'mpv').resolve()), '-c', 'core.autocrlf=true', 'diff', '--no-ext-diff', '--binary', 'HEAD', '--'])
    if actual_patch != patch:
        raise ValueError('Local mpv sources do not match the shipped subtitle patch')
    version = re.search(r'^version = "([0-9.]+)"', (app/'Cargo.toml').read_text(), re.M)[1]
    output = args.destination.resolve()
    output.mkdir(parents=True, exist_ok=True)
    names = [f'Enigmax-CinePlayer-{kind}-{version}.zip' for kind in ('Sources', 'Dependency-Sources')]
    for name in names:
        if (output/name).exists():
            raise FileExistsError(output/name)
    lock = json.loads((args.pinned/'dependency-archives/sources.lock.json').read_text())
    # Neither component is configured/linked; exclude their huge unused data sets.
    selected = [p for p in lock['packages'] if not p['archive'].startswith(('mingw-w64-whisper.cpp-', 'mingw-w64-libmysofa-'))]
    manifest = json.loads((app/'licencias/engine-manifest.txt').read_text())
    package_names = {name for p in selected for name in p['binaryPackages']}
    for module in manifest['modules']:
        if module.get('package') and module['package'] not in package_names:
            raise ValueError(f'Missing corresponding source for {module["path"]}')
    for p in selected:
        path = args.pinned/'dependency-archives'/p['archive']
        if path.stat().st_size != p['bytes'] or digest(path) != p['sha256']:
            raise ValueError(f'Source archive changed: {path}')
    deps = args.reuse_dependencies.resolve() if args.reuse_dependencies else output/names[1]
    if args.reuse_dependencies:
        if not args.reuse_sha256 or digest(deps) != args.reuse_sha256:
            raise ValueError('Dependency archive checksum mismatch')
        with zipfile.ZipFile(deps) as archive:
            recorded=json.loads(archive.read('dependency-sources/sources.lock.json'))
            if recorded != {'format':1,'packages':selected} or archive.testzip() is not None:
                raise ValueError('Dependency archive does not correspond to these sources')
    else:
        with zipfile.ZipFile(deps, 'x', compression=zipfile.ZIP_STORED, allowZip64=True, strict_timestamps=False) as archive:
            for p in selected:
                archive.write(args.pinned/'dependency-archives'/p['archive'], 'dependency-sources/'+p['archive'])
            add_tree(archive, args.pinned/'dependency-archives/metadata', 'dependency-sources/metadata')
            archive.writestr('dependency-sources/sources.lock.json', json.dumps({'format':1,'packages':selected},indent=2))
    print(f'{deps.name}: {deps.stat().st_size} bytes, {len(selected)} source archives', flush=True)
    src = output/names[0]
    with zipfile.ZipFile(src, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=6, allowZip64=True, strict_timestamps=False) as archive:
        tops = ['Cargo.toml','Cargo.lock','build.rs','build.ps1','build-native.ps1','build-portable.ps1',
                '.gitignore','README.md','LICENSE.txt','DEPENDENCIAS.md','COMPILAR.md','FUENTES.md',
                'ACTUALIZACIONES.md','INSTALACION.txt','GOOGLETV.md','verificar.ps1','verificar-ass3d.ps1']
        report = f"VERIFICACION_{version}.md"
        if (app/report).is_file():
            tops.append(report)
        for name in tops:
            archive.write(app/name, 'fuentes/EnigmaxCinePlayer/'+name)
        for name in ('ass3d-sbs.ass','ass3d-ou.ass'):
            archive.write(app/'verificacion/muestras'/name,'fuentes/EnigmaxCinePlayer/verificacion/muestras/'+name)
        for directory in ('src','native','assets','licencias','.github','verificacion/tests'):
            root = app/directory
            if not root.exists():
                continue
            for path in sorted(root.rglob('*')):
                if not path.is_file():
                    continue
                rel = path.relative_to(app)
                if any(p in ('.git','__pycache__','target','build') for p in rel.parts):
                    continue
                if path.suffix.lower() in ('.mkv','.mp4','.iso','.exe','.dll','.o','.pyc'):
                    continue
                if path.suffix.lower() in ('.png','.jpg') and not rel.as_posix().startswith('assets/'):
                    continue
                if path.suffix.lower() == '.a' and not rel.as_posix().startswith('native/vendor/ffmpeg/lib/'):
                    continue
                archive.write(path, 'fuentes/EnigmaxCinePlayer/'+rel.as_posix())
        for name, sha in (('mpv','dc4b285c93043d99492c3664a24f4ae08cdfdf0d'),('ffmpeg','6882969ce9fd2aa6aa1d0008b2bfe1c838aac11d')):
            if subprocess.check_output([str(args.git),'--git-dir='+str((args.pinned/name/'.git').resolve()),'rev-parse','HEAD'],text=True).strip()!=sha:
                raise ValueError('Pinned checkout changed')
            data = subprocess.check_output([str(args.git),'-C',str((args.pinned/name).resolve()),'archive','--format=tar',sha])
            with tarfile.open(fileobj=io.BytesIO(data)) as snapshot:
                for entry in snapshot:
                    if entry.isfile():
                        changed = name == 'mpv' and entry.name in ('options/options.c','options/options.h','sub/osd.c','sub/osd.h','sub/osd_state.h','video/out/gpu/osd.c','video/out/vo_gpu_next.c')
                        content = (args.pinned/name/entry.name).read_bytes() if changed else snapshot.extractfile(entry).read()
                        archive.writestr('fuentes/'+name+'/'+entry.name, content)
            archive.writestr('fuentes/'+name+'/.source-revision',sha+'\n')
            if name == 'mpv':
                archive.writestr('fuentes/mpv/.cineplayer-patch-sha256',hashlib.sha256(patch).hexdigest()+'\n')
        add_tree(archive, args.pinned/'cargo-vendor', 'fuentes/cargo-vendor')
        add_tree(archive, args.pinned/'rust-upstreams', 'fuentes/rust-upstreams')
        archive.write(args.pinned/'msys2-packages.lock','fuentes/msys2-packages.lock')
    if args.reuse_dependencies:
        with zipfile.ZipFile(src) as archive:
            if archive.testzip() is not None:
                raise ValueError('Invalid player source archive')
        deps.rename(output/names[1])
    checks = []
    for name in names:
        path=output/name
        with zipfile.ZipFile(path) as archive:
            bad=archive.testzip()
            if bad:
                raise ValueError(f'Corrupt ZIP entry: {bad}')
        value=digest(path)
        checks.append(f'{value}  {name}')
        print(f'{name}: {path.stat().st_size} bytes, SHA256 {value}',flush=True)
    (output/f'Enigmax-CinePlayer-Sources-{version}.sha256').write_text('\n'.join(checks)+'\n',encoding='utf-8')


if __name__ == '__main__':
    main()
