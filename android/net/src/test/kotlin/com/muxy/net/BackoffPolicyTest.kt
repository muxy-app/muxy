package com.muxy.net

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.random.Random

class BackoffPolicyTest {

    @Test
    fun `attempt 0 is zero`() {
        val policy = BackoffPolicy(baseMs = 100, maxMs = 5_000, jitterMs = 0, random = Random(0))
        assertEquals(0, policy.delayForAttempt(0))
    }

    @Test
    fun `exponential growth with cap`() {
        val policy = BackoffPolicy(baseMs = 100, maxMs = 1_000, jitterMs = 0, random = Random(0))
        assertEquals(100, policy.delayForAttempt(1))
        assertEquals(200, policy.delayForAttempt(2))
        assertEquals(400, policy.delayForAttempt(3))
        assertEquals(800, policy.delayForAttempt(4))
        assertEquals(1_000, policy.delayForAttempt(5))
    }

    @Test
    fun `jitter adds random delay within range`() {
        val policy = BackoffPolicy(baseMs = 100, maxMs = 5_000, jitterMs = 50, random = Random(42))
        repeat(20) { i ->
            val d = policy.delayForAttempt(1)
            assertTrue("delay $d out of range", d in 100..149)
        }
    }
}
