package com.muxy.net

import com.muxy.protocol.envelope.MuxyMethod
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds

object MuxyTimeouts {
    val default: Duration = 10.seconds

    fun forMethod(method: MuxyMethod): Duration =
        when (method) {
            MuxyMethod.PAIR_DEVICE -> 120.seconds
            MuxyMethod.VCS_COMMIT -> 60.seconds
            MuxyMethod.VCS_PUSH, MuxyMethod.VCS_PULL, MuxyMethod.VCS_CREATE_PR -> 120.seconds
            MuxyMethod.VCS_SWITCH_BRANCH, MuxyMethod.VCS_CREATE_BRANCH -> 30.seconds
            MuxyMethod.VCS_ADD_WORKTREE, MuxyMethod.VCS_REMOVE_WORKTREE -> 60.seconds
            else -> default
        }

    val voidMethods: Set<MuxyMethod> = setOf(MuxyMethod.TERMINAL_INPUT)
}
