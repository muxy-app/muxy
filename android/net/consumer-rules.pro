-keepattributes *Annotation*

-if @kotlinx.serialization.Serializable class **
-keepclassmembers class <1> {
    static <1>$Companion Companion;
    private static final ** $cachedSerializer$delegate;
}

-keep,includedescriptorclasses class com.muxy.**$$serializer { *; }
-keepclassmembers class com.muxy.** {
    *** Companion;
}
-keepclasseswithmembers class com.muxy.** {
    kotlinx.serialization.KSerializer serializer(...);
}
