if [[ -n "${GHOSTTY_ZSH_ZDOTDIR+X}" ]]; then
    builtin export ZDOTDIR="$GHOSTTY_ZSH_ZDOTDIR"
    builtin unset GHOSTTY_ZSH_ZDOTDIR
else
    builtin unset ZDOTDIR
fi

_muxy_user_zshenv=${ZDOTDIR-$HOME}/.zshenv
if [[ -r "$_muxy_user_zshenv" ]]; then
    builtin source -- "$_muxy_user_zshenv"
fi
builtin unset _muxy_user_zshenv

if [[ -o interactive ]]; then
    _muxy_integration="${${(%):-%x}:A:h}/ghostty-integration"
    if [[ -r "$_muxy_integration" ]]; then
        builtin source -- "$_muxy_integration"
    fi
    builtin unset _muxy_integration
fi
