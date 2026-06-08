#!/usr/bin/expect -f
set timeout 30
set user $env(MUXY_SSH_USER)
set host $env(MUXY_SSH_HOST)
set port $env(MUXY_SSH_PORT)
set remote_path $env(MUXY_SSH_REMOTE_PATH)
set password $env(MUXY_SSH_PASSWORD)
spawn -noecho ssh -o ControlMaster=auto -o ControlPath=$env(MUXY_SSH_CONTROL_PATH) -o ControlPersist=10m -p $port $user@$host -t "cd $remote_path; exec \$SHELL -l"
expect {
    -re "password:" {
        send -- "$password\r"
        exp_continue
    }
    -re "yes/no" {
        send -- "yes\r"
        exp_continue
    }
    timeout {
        exit 1
    }
    eof {
        exit 1
    }
}
interact
