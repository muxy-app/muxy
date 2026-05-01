package com.muxy.terminal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ModifierTransformTest {

    @Test
    fun ctrlLowerLetterMapsToControlByte() {
        assertEquals("\u0001", ModifierTransform.transform("a", ArmedModifier.CTRL))
        assertEquals("\u0003", ModifierTransform.transform("c", ArmedModifier.CTRL))
        assertEquals("\u001A", ModifierTransform.transform("z", ArmedModifier.CTRL))
    }

    @Test
    fun ctrlUpperLetterMapsToControlByte() {
        assertEquals("\u0001", ModifierTransform.transform("A", ArmedModifier.CTRL))
        assertEquals("\u0003", ModifierTransform.transform("C", ArmedModifier.CTRL))
        assertEquals("\u001F", ModifierTransform.transform("_", ArmedModifier.CTRL))
    }

    @Test
    fun ctrlSpaceMapsToNul() {
        assertEquals("\u0000", ModifierTransform.transform(" ", ArmedModifier.CTRL))
    }

    @Test
    fun ctrlOtherCharactersReturnNull() {
        assertNull(ModifierTransform.transform("ab", ArmedModifier.CTRL))
        assertNull(ModifierTransform.transform("1", ArmedModifier.CTRL))
    }

    @Test
    fun shiftUppercases() {
        assertEquals("FOO", ModifierTransform.transform("foo", ArmedModifier.SHIFT))
        assertEquals("X", ModifierTransform.transform("x", ArmedModifier.SHIFT))
    }

    @Test
    fun altPrependsEsc() {
        assertEquals("\u001Bb", ModifierTransform.transform("b", ArmedModifier.ALT))
        assertEquals("\u001Bff", ModifierTransform.transform("ff", ArmedModifier.ALT))
    }

    @Test
    fun cmdPassesThrough() {
        assertEquals("z", ModifierTransform.transform("z", ArmedModifier.CMD))
    }
}
