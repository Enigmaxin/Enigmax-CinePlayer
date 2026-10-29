#!/usr/bin/env bash
# Native Windows/UCRT64 build. Every source checkout must match its recorded SHA.
# Dependencies are supplied by a separately recorded, signature-verified MSYS2 set.
set -euo pipefail
source_root=$(realpath "${1:?source directory required}")
player_native=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
build_root=$(realpath -m "${2:?build directory required}")
jobs=${3:-4}
stage=${4:-all}
[[ "$stage" == all || "$stage" == ffmpeg || "$stage" == mpv ]] || exit 1
[[ "$MSYSTEM" == UCRT64 ]] || { echo 'Run in the MSYS2 UCRT64 environment' >&2; exit 1; }
[[ "$jobs" =~ ^[1-9][0-9]?$ ]] && (( jobs <= 16 )) || { echo 'Invalid parallelism' >&2; exit 1; }
verify_source() {
  local path="$1" expected="$2"
  if [[ -d "$path/.git" ]]; then
    [[ $(git -C "$path" rev-parse HEAD) == "$expected" ]]
    # Windows checkouts may use CRLF without changing the pinned source content.
    if ! git -C "$path" -c core.autocrlf=true diff --quiet HEAD --; then
      [[ $(basename "$path") == mpv ]]
      git -C "$path" -c core.autocrlf=true diff --no-ext-diff --binary HEAD -- |
        cmp - "$player_native/mpv-subtitle-stereo.patch"
    fi
  else
    # These snapshots are exported with git archive; verify their release ZIP SHA-256 first.
    [[ $(cat "$path/.source-revision") == "$expected" ]]
  fi
}
verify_source "$source_root/mpv" dc4b285c93043d99492c3664a24f4ae08cdfdf0d
verify_source "$source_root/ffmpeg" 6882969ce9fd2aa6aa1d0008b2bfe1c838aac11d
mkdir -p "$build_root/ffmpeg" "$build_root/prefix"
prefix="$build_root/prefix"
export PKG_CONFIG_PATH="$prefix/lib/pkgconfig:/ucrt64/lib/pkgconfig:/ucrt64/share/pkgconfig"
export PATH="$prefix/bin:/ucrt64/bin:/usr/bin:$PATH"
export CFLAGS='-O2 -march=x86-64 -mtune=generic'
export CXXFLAGS="$CFLAGS"

if [[ "$stage" != mpv ]]; then
cd "$build_root/ffmpeg"
"$source_root/ffmpeg/configure" \
  --prefix="$prefix" --target-os=mingw32 --arch=x86_64 --cc=gcc --cxx=g++ \
  --enable-shared --disable-static --disable-programs --disable-doc --disable-debug \
  --enable-gpl --enable-version3 --enable-runtime-cpudetect \
  --enable-dxva2 --enable-d3d11va --enable-d3d12va \
  --enable-gnutls --enable-gmp --enable-iconv --enable-zlib \
  --enable-libaom --enable-libdav1d --enable-libvpx --enable-libopenjpeg \
  --enable-libass --enable-libfontconfig --enable-libfreetype --enable-libfribidi \
  --enable-libharfbuzz --enable-libbluray --enable-libgme --enable-libmodplug \
  --enable-libgsm --enable-libmp3lame --enable-libopencore_amrnb --enable-libopencore_amrwb \
  --enable-libopus --enable-libspeex --enable-libtheora --enable-libvorbis \
  --enable-libsoxr --enable-libzimg --enable-libxml2 --enable-libjxl --enable-libwebp \
  --enable-libssh --enable-librtmp --enable-libsrt \
  --enable-libx264 --enable-libx265 --enable-libxvid --enable-libvidstab \
  --enable-libzvbi --enable-liblc3 --enable-libplacebo --enable-librav1e \
  --enable-librsvg --enable-libsvtav1 --enable-libvpl --enable-vulkan \
  --extra-cflags="$CFLAGS" --extra-cxxflags="$CXXFLAGS"
make -j"$jobs"
make install
fi
[[ "$stage" != ffmpeg ]] || exit 0

meson_args=(setup)
[[ ! -f "$build_root/mpv/meson-private/coredata.dat" ]] || meson_args+=(--reconfigure)
meson "${meson_args[@]}" "$build_root/mpv" "$source_root/mpv" \
  --prefix="$prefix" --buildtype=release --default-library=shared --wrap-mode=nodownload \
  -Doptimization=2 -Db_lto=true -Dbuild-date=false -Dcplayer=false -Dlibmpv=true \
  -Dlua=lua5.1 -Dd3d11=enabled -Dwasapi=enabled -Dlibbluray=enabled \
  -Dlcms2=enabled -Dlibarchive=enabled -Duchardet=enabled -Dzimg=enabled \
  -Dshaderc=enabled -Dspirv-cross=enabled -Dwin32-smtc=disabled \
  -Djavascript=disabled -Dvapoursynth=disabled
meson compile -C "$build_root/mpv" -j "$jobs"
meson install -C "$build_root/mpv"
printf '%s\n' 'Engine compilation finished; staged libraries have NOT replaced the player runtime.'
