# VAULT

– In active development, not ready yet for production –

Vault is a simple cross-platform password manager built in pure rust and
utilizing [floem](https://github.com/lapce/floem) for the UI.

### How to try it
```
git clone https://github.com/dominikwilkowski/vault.git
cd vault
cargo run
```

### Configuration
Both the password database and configuration are in `vault_config.toml` currently
this is stored in the current working directory. If one does not exist it will be
created.  

### Screenshots
![image](assets/password.png)

![image](assets/detail_view.png)

### Notes
Verify memory is clean when locked:
```sh
λ ps -e|grep vault
λ lldb --attach-pid <pip>
(lldb) process save-core <path>
(lldb) exit
λ cat <path> | strings | grep totally_secure_password
```

## License
Copyleft (c) 2023
Licensed under the [GNU GPL-3.0-or-later](https://github.com/dominikwilkowski/vault/blob/main/LICENSE).
