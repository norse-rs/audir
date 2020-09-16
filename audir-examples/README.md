# audir-examples

## music

Playback an audio file in a loop.
Running on Android requires a file named `asmr_48000.ogg` placed in `assets` directory.

#### Desktop:
```
cargo run --example desktop --features music -- <file.ogg>
```

#### Android (AAudio):
```
cargo apk run --example android --features "aaudio music"
```

#### Android (OpenSLES):
```
cargo apk run --example android --features "opensles music"
```

