[[ -o interactive && ${MUXY_SHELL_INTEGRATION-} == 1 ]] || return
(( ${+_muxy_installed} )) && return
typeset -g _muxy_installed=1 _muxy_running=0 _muxy_prompt='' _muxy_user_prompt=''

_muxy_directory() {
    emulate -L zsh
    local LC_ALL=C encoded='' char hex
    local -i i
    for (( i=1; i<=${#PWD}; i++ )); do
        char=$PWD[i]
        if [[ $char == [a-zA-Z0-9/._~-] ]]; then
            encoded+=$char
        else
            printf -v hex '%%%02X' "'$char"
            encoded+=$hex
        fi
    done
    printf '\e]7;file://%s\a' "$encoded"
}

_muxy_precmd() {
    local result=$?
    if (( _muxy_running )); then
        printf '\e]133;D;%d\a' "$result"
    fi
    _muxy_running=0
    _muxy_directory
    [[ $PS1 != $_muxy_prompt ]] && _muxy_user_prompt=$PS1
    _muxy_prompt=$'%{\e]133;A\a%}'"$_muxy_user_prompt"$'%{\e]133;B\a%}'
    PS1=$_muxy_prompt
    return "$result"
}

_muxy_preexec() {
    _muxy_running=1
    printf '\e]133;C\a'
}

_muxy_unbound_alt() { return 0 }

_muxy_install() {
    local result=$?
    precmd_functions=(${precmd_functions:#_muxy_install} _muxy_precmd)
    preexec_functions+=(_muxy_preexec)
    zle -N _muxy_unbound_alt
    local keymap sequence binding
    for keymap in emacs viins vicmd; do
        for sequence in $'\e[1;3A' $'\e[1;3B'; do
            binding=$(bindkey -M "$keymap" "$sequence")
            if [[ ${binding##* } == undefined-key ]]; then
                bindkey -M "$keymap" "$sequence" _muxy_unbound_alt
            fi
        done
    done
    _muxy_precmd
    return "$result"
}
precmd_functions+=(_muxy_install)
