# Source explicitly from your interactive Bash startup file. Bash startup is not redirected.
[[ $- == *i* && ${MUXY_SHELL_INTEGRATION-} == 1 ]] || return
[[ ${_muxy_installed-} == 1 ]] && return
_muxy_installed=1
_muxy_running=0
_muxy_in_prompt=0
_muxy_debug_checked=0
_muxy_prompt=''
_muxy_user_prompt=''

_muxy_directory() {
    local LC_ALL=C encoded='' char hex i
    for (( i=0; i<${#PWD}; i++ )); do
        char=${PWD:i:1}
        case $char in
            [a-zA-Z0-9/._~-]) encoded+=$char ;;
            *) printf -v hex '%%%02X' "'$char"; encoded+=$hex ;;
        esac
    done
    printf '\e]7;file://%s\a' "$encoded"
}

_muxy_precmd() {
    local result=$?
    if (( _muxy_running )); then
        printf '\e]133;D;%d\a' "$result"
    fi
    _muxy_running=0
    _muxy_in_prompt=1
    _muxy_status=$result
    return "$result"
}

_muxy_prompt_end() {
    _muxy_directory
    [[ $PS1 != "$_muxy_prompt" ]] && _muxy_user_prompt=$PS1
    _muxy_prompt=$'\[\e]133;A\a\]'"$_muxy_user_prompt"$'\[\e]133;B\a\]'
    PS1=$_muxy_prompt
    _muxy_in_prompt=0
    return "$_muxy_status"
}

_muxy_prompt_status() {
    return "$_muxy_status"
}

# Bash 5 hides the caller's DEBUG trap while sourcing. Inspect it afterward,
# directly in PROMPT_COMMAND, without replacing a user's trap.
read -r -d '' _muxy_debug_setup <<'MUXY_DEBUG_SETUP' || :
if [[ ${_muxy_debug_checked-0} == 0 ]]; then
    _muxy_debug_checked=1
    if [[ -z $(trap -p DEBUG) ]]; then
        trap 'if [[ $_muxy_running == 0 && $_muxy_in_prompt == 0 && $BASH_COMMAND != _muxy_* && $BASH_COMMAND != "$PROMPT_COMMAND" ]]; then _muxy_running=1; printf "\e]133;C\a"; fi' DEBUG
    fi
fi
_muxy_prompt_status
MUXY_DEBUG_SETUP
if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
    PROMPT_COMMAND=(_muxy_precmd "$_muxy_debug_setup" "${PROMPT_COMMAND[@]}" _muxy_prompt_end)
else
    PROMPT_COMMAND=$'_muxy_precmd\n'"$_muxy_debug_setup"$'\n'"${PROMPT_COMMAND-}"$'\n_muxy_prompt_end'
fi

_muxy_bind_keys() {
    local keymap sequence bindings
    for keymap in emacs-standard vi-insert vi-command; do
        bindings=$(bind -m "$keymap" -p; bind -m "$keymap" -s)
        for sequence in '\e[1;3A' '\e[1;3B'; do
            if [[ $bindings != *"\"$sequence\":"* ]]; then
                bind -m "$keymap" "\"$sequence\": \"\""
            fi
        done
    done
}
_muxy_bind_keys
unset -f _muxy_bind_keys
