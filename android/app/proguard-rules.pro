-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt

-keep,includedescriptorclasses class com.muxy.**$$serializer { *; }
-keepclassmembers class com.muxy.** {
    *** Companion;
}
-keepclasseswithmembers class com.muxy.** {
    kotlinx.serialization.KSerializer serializer(...);
}

-if @kotlinx.serialization.Serializable class **
-keepclassmembers class <1> {
    static <1>$Companion Companion;
    private static final ** $cachedSerializer$delegate;
}

-keep class kotlinx.serialization.** { *; }
-keep class kotlinx.serialization.json.** { *; }
-keep class kotlinx.serialization.internal.** { *; }

-keep class com.termux.terminal.** { *; }
-keep class com.termux.view.** { *; }
-keep class com.muxy.terminal.MuxyTerminalSession { *; }
