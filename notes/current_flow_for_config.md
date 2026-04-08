# current flow for first run as user

1. user runs adamantite live
2. adamantite checks for config
  2.a if adamantite finds config, deserialize config using toml crate, populate struct, pass that to find_pid function
3. if does not exist, create a default config using serde and toml
4. then prompt find pid and continue as usual
