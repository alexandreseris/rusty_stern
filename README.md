# rusty_stern
rust implementation of https://github.com/stern/stern

## Build
```sh
cargo +nightly build
```

currently the program needs (more or less) the env var KUBECONFIG with the path to the target kubernetes config file

## Left to do
- lock stdout
- deal with color picking
- padding according to the pod's name
- CLI (model from actual stern)
- custom kubeconfig file
- build and publish as release
- documentation :3
