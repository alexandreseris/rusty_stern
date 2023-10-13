# rusty_stern

Rust implementation of <https://github.com/stern/stern>

=> _Allow you to tail multiple pods log on Kubernetes_

## Differences with regular stern

- log lines printed are not mixed (stdout is locked each print)
- the pod search is refreshed every n seconds. If new pods are added, you don't need to restart the command
- some control over colors used to display pods name

## Usage

```text
Usage: rusty_stern.exe [OPTIONS]

Options:
  -p, --pod-search <reg pattern>   regex to match pod names [default: .+]
  -k, --kubeconfig <filepath>      path to the kubeconfig file. if the option is not passed, try to infer configuration [default: ]
  -n, --namespaces <nmspc>         kubernetes namespaces to use separated by commas [default: default]
      --previous                   retrieve previous terminated container logs
      --since-seconds <seconds>    a relative time in seconds before the current time from which to show logs
      --tail-lines <line_cnt>      number of lines from the end of the logs to show
      --timestamps                 show timestamp at the begining of each log line
      --loop-pause <seconds>       number of seconds between each pod list query (doesn't affect log line display) [default: 2]
      --hue-intervals <intervals>  hue (hsl) intervals to pick for color cycle generation format is $start-$end(,$start-$end)* where $start>=0 and $end<=359 eg for powershell: 0-180,280-359 [default: 0-359]
      --color-saturation <sat>     the color saturation (0-100) [default: 100]
      --color-lightness <light>    the color lightness (0-100) [default: 50]
      --filter <filter>            regex string to filter output that match [default: ]
      --inv-filter <inv_filter>    regex string to filter output that does not match [default: ]
      --replace-pattern <pattern>  regex string to replace pattern (pattern part) [default: ]
      --replace-value <value>      string to replace the pattern captured (or not) by replace_pattern check documentation if needed at https://docs.rs/regex/1.3.3/regex/struct.Regex.html#replacement-string-syntax [default: ]
  -h, --help                       Print help
  -V, --version                    Print version
```

## Build

```sh
# add nightly toolchain
rustup toolchain install nightly

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
apt install libssl-dev
## Windows (using powershell)
# install https://github.com/microsoft/vcpkg (check readme)
vcpkg install openssl --triplet=x64-windows-static
$vcpkgloc = "vcpkg_installation_path"
$env:OPENSSL_DIR = "$vcpkgloc\installed\x64-windows-static"

# build for linux 64
cargo +nightly build --bin rusty_stern --release --target x86_64-unknown-linux-gnu
# build for windows 64
cargo +nightly build --bin rusty_stern --release --target x86_64-pc-windows-msvc
```
