cargo rustc --release -- -C link-args=-Wl,--subsystem,windows
mkdir dist
copy target\release\*.exe .\dist
copy SDL2.dll .\dist

