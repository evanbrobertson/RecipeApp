# kotlinx.serialization keeps generated serializers through its bundled rules.
# Navigation routes are @Serializable classes looked up by name.
-keep @kotlinx.serialization.Serializable class app.crumb.android.** { *; }
