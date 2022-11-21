# rusty_stern
Rust implementation of https://github.com/stern/stern

=> _Allow you to tail multiple pods log on Kubernetes_

### Differences with regular stern:
- log lines printed are not mixed (stdout is locked each print)
- the pod search is refreshed every n seconds. If new pods are added, you don't need to restart the command
- some control over colors used to display pods name

## Build
```sh
# debug
cargo +nightly build

# get target architecture installed:
rustup target list --installed
# list all target
rustc --print target-list
# add a target
rustup target add targetname --toolchain nightly

# build dependencies
## Linux
libssl-dev
## Windows
about 1 millions things

# build for linux 64
cargo +nightly build --bin rusty_stern --release --target x86_64-unknown-linux-gnu
# build for windows 64
cargo +nightly build --bin rusty_stern --release --target x86_64-pc-windows-gnu
### infinite pain, check https://docs.rs/openssl/latest/openssl/#automatic
### or maybe https://docs.rs/crate/openssl/0.2.6
```

## Usage
_every option mentionned here can be used in a json utf8 config file (use long option name and snake case instead of kebab case) located in $HOME/rusty_stern/config_

_command line arguments have priority over config file settings_

```
Usage: rusty_stern [OPTIONS]

Options:
  -p, --pod-search <reg pattern>  regex to match pod names [default: .+]
  -k, --kubeconfig <filepath>     path to the kubeconfig file. if the option is not passed, try to infer configuration [default: ]
  -n, --namespace <nmspc>         kubernetes namespace to use. if the option is not passed, use the default namespace [default: ]
      --previous                  retrieve previous terminated container logs
      --since-seconds <seconds>   a relative time in seconds before the current time from which to show logs [default: 0]
      --tail-lines <line_cnt>     number of lines from the end of the logs to show [default: 0]
      --timestamps                show timestamp at the begining of each log line
      --loop-pause <seconds>      number of seconds between each pod list query (doesn't affect log line display) [default: 2]
  -v, --verbose                   verbose output
      --debug-color <rgb>         debug rgb color (format is 0-255,0-255,0-255) [default: 255,255,255]
      --color-cycle-len <num>     number of color to generate for the color cycle. if 0, it is later set for about the number of result retuned by the first pod search [default: 0]
      --color-saturation <sat>    the color saturation (0-100) [default: 100]
      --color-lightness <light>   the color lightness (0-100) [default: 50]
  -h, --help                      Print help information
  -V, --version                   Print version information
```

## Left to do
- multiple namespace
- build (for windows) and publish as release
- proper error handling :O
- documentation :3
