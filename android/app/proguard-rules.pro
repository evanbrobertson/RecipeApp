# kotlinx.serialization keeps generated serializers through its bundled rules.
# Navigation routes are @Serializable classes looked up by name.
-keep @kotlinx.serialization.Serializable class app.crumb.android.** { *; }

# crumb-core bindings: JNA reaches the generated structures and callbacks by reflection.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keep class app.crumb.core.** { *; }
-dontwarn java.awt.**
