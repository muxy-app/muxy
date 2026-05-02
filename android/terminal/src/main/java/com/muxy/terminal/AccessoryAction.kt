package com.muxy.terminal

interface AccessoryActions {
    fun sendText(text: String)

    fun pasteFromClipboard()

    fun toggleKeyboard()
}
