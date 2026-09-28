package app.crumb.android.ui.connect

import org.junit.Assert.assertEquals
import org.junit.Test

class ConnectLogicTest {
    @Test
    fun claudeCodeCommandUsesTheConnectorUrl() {
        assertEquals(
            "claude mcp add --transport http recipes https://crumb.example.com/mcp",
            claudeCommand("https://crumb.example.com/mcp"),
        )
    }
}
