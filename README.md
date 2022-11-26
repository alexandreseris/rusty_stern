# rusty_stern

Rust implementation of <https://github.com/stern/stern>

=> _Allow you to tail multiple pods log on Kubernetes_

## Differences with regular stern

- log lines printed are not mixed (stdout is locked each print)
- the pod search is refreshed every n seconds. If new pods are added, you don't need to restart the command
- some control over colors used to display pods name

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

## Usage

every option mentionned here can be used in a json utf8 config file (use long option name and snake case instead of kebab case) located in $HOME/rusty_stern/config

you can use the option --generate-config-file to generate a default config file for convenience

command line arguments have priority over config file settings

```text
Usage: rusty_stern.exe [OPTIONS]

Options:
  -p, --pod-search <reg pattern>   regex to match pod names [default: .+]
  -k, --kubeconfig <filepath>      path to the kubeconfig file. if the option is not passed, try to infer configuration [default: ]
  -n, --namespace <nmspc>          kubernetes namespace to use. if the option is not passed, use the default namespace
      --previous                   retrieve previous terminated container logs
      --since-seconds <seconds>    a relative time in seconds before the current time from which to show logs [default: 0]
      --tail-lines <line_cnt>      number of lines from the end of the logs to show [default: 0]
      --timestamps                 show timestamp at the begining of each log line
      --disable-pods-refresh       disable automatic pod list refresh
      --loop-pause <seconds>       number of seconds between each pod list query (doesn't affect log line display) [default: 2]
      --default-color <hsl>        default hsl color (format is hue,saturation,lightness), used for general and error messages default hsl color (format is hue,saturation,lightness) [default: 0,0,100]
      --color-cycle-len <num>      number of color to generate for the color cycle. if 0, it is later set for about the number of result retuned by the first pod search [default: 0]
      --hue-intervals <intervals>  hue (hsl) intervals to pick for color cycle generation format is $start-$end(,$start-$end)* where $start>=0 and $end<=359 eg for powershell: 0-180,280-359 [default: 0-359]
      --color-saturation <sat>     the color saturation (0-100) [default: 100]
      --color-lightness <light>    the color lightness (0-100) [default: 50]
      --generate-config-file       generate a default config file and exit
  -h, --help                       Print help information
  -V, --version                    Print version information
```

## Left to do

- changing default color does not work
- json config file is fucked up
