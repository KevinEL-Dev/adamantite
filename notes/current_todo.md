# config template
```toml
[search_method]
method = "systemd"
unit_name = "hytale"
```
# todo

to configure serde and toml for adamantite


## todo 2026-04-09

serialize a default config

write this to config.toml (search up how to write a file)

fs::write


### enable user to configure config easily through cli

adamantite config method "systemd"
adamantite config unitname "hytale"


### learning how to set up ci/cd for adamantite


continous integration and continous deployment.

what exactly needs to be done within this ci/cd pipeline

merge dev branch into main

every commit into the dev branch should be tested, if pass, merge to main.

create a new version within cargo.toml

create a new tag for release in git
v0.2.x
push that tag to origin

cargo build --release

mkdir adamantite-v<current_version>-unknown-linux-gnu

cp target/release/adamantite adamantite-v<current_version>-unknown-linux-gnu/
cp README.md adamantite-v<current_version>-unknown-linux-gnu/
cp LICENSE adamantite-v<current_version>-unknown-linux-gnu/

tar czf adamantite-v<current_version>-unknown-linux-gnu.tar.gz adamantite-v<current_version>-unknown-linux-gnu/

sha256sum adamantite-v<current_version>-unknown-linux-gnu.tar.gz > SHA256SUMS 

the github release will have these files 

1. adamantite-v<current_version>-unknown-linux-gnu.tar.gz
2. SHA256SUMS

i will go and set the new updates on the release

it should also run cargo publish


getting another diet coke





