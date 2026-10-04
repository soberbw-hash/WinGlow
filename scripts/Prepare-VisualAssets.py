from pathlib import Path
import hashlib, zipfile
app = Path(__file__).resolve().parents[1] / 'WinGlow.App'
src = app / 'output/integration-sources'
dest = app.parent / 'third-party/Windhawk'
dest.mkdir(parents=True, exist_ok=True)
assert hashlib.sha256((src/'windhawk_setup_offline.exe').read_bytes()).hexdigest() == '4d93016570f982326eebdfc9068e924ff21f6448534ea32672bb4c1d52a8193b'
assert hashlib.sha256((src/'start-menu-1.7-x64.dll').read_bytes()).hexdigest() == 'c116a5388ac8299ca9098e139bddff496ab025d75fe0652f607767904dd89f4d'
def put(z, source, name):
    data = source.read_bytes() if isinstance(source, Path) else source.encode()
    info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = 0o100644 << 16
    z.writestr(info, data)

with zipfile.ZipFile(dest/'Windhawk-1.7.3-start-menu-1.7.zip', 'w', zipfile.ZIP_DEFLATED) as z:
    pkg=src/'windhawk-package'
    for name in ['windhawk.exe', 'windhawk-x64-helper.exe']:
        put(z, pkg/name, name)
    for arch in ['32','64']:
        put(z, pkg/f'AppData/Engine/Mods/{arch}/windhawk-mod-shim.dll', f'AppData/Engine/Mods/{arch}/windhawk-mod-shim.dll')
        triple = 'i686-w64-mingw32' if arch == '32' else 'x86_64-w64-mingw32'
        for lib in ['libc++', 'libunwind']:
            put(z, pkg/f'Compiler/{triple}/bin/{lib}.dll', f'AppData/Engine/Mods/{arch}/{lib}.whl')
        for p in sorted((pkg/'Engine/$R1'/arch).glob('*')):
            if p.suffix.lower() == '.dll' or p.name == 'symsrv.yes':
                put(z, p, f'Engine/1.7.3/{arch}/{p.name}')
    put(z, src/'start-menu-1.7-x64.dll', 'AppData/Engine/Mods/64/windows-11-start-menu-styler_1.7.dll')
    put(z, src/'start-menu.wh.cpp', 'AppData/ModsSource/windows-11-start-menu-styler.wh.cpp')
    put(z, src/'windhawk/LICENSE', 'LICENSE')
    put(z, pkg/'Compiler/LICENSE.TXT', 'LLVM-LICENSE.txt')
    put(z, '[Storage]\r\nPortable=1\r\nAppDataPath=AppData\r\nEnginePath=Engine/1.7.3\r\nCompilerPath=Compiler\r\nUIPath=UI\r\n', 'windhawk.ini')
    put(z, '[Storage]\r\nPortable=1\r\nAppDataPath=../../AppData/Engine\r\n', 'Engine/1.7.3/engine.ini')
print(hashlib.sha256((dest/'Windhawk-1.7.3-start-menu-1.7.zip').read_bytes()).hexdigest())
