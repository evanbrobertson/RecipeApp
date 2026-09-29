package app.crumb.android.data

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AccountParseTest {
    private fun j(text: String) = Json.parseToJsonElement(text)

    @Test
    fun accountsHouseholdMarksYouAndNumbersBecomeIds() {
        val h = AccountParse.accountsHousehold(
            j(
                """{"id":1,"name":"Home","role":"owner","you":7,
                "members":[{"userId":7,"name":"Sam","email":"s@x.io","role":"owner"},{"userId":9,"name":"Al","email":"a@x.io","role":"member"}],
                "invites":[{"id":3,"createdBy":"Sam","expiresAt":1800000000}],
                "households":[{"id":1,"name":"Home"},{"id":2,"name":"Cabin"}]}""",
            ),
        )
        assertEquals("1", h.id)
        assertTrue(h.isOwner)
        assertEquals(listOf(true, false), h.members.map { it.you })
        assertEquals("9", h.members[1].id)
        assertNull(h.invites.single().email)
        assertEquals("Sam", h.invites.single().createdBy)
        assertEquals(listOf("Cabin"), h.others.map { it.name })
    }

    @Test
    fun hostedHouseholdKeepsOnlyLiveInvitesAndListsOwnersFirst() {
        val org = j(
            """{"id":"org1","name":"Home",
            "members":[{"id":"m2","userId":"u2","role":"member","user":{"name":"Al","email":"a@x.io"}},{"id":"m1","userId":"u1","role":"owner","user":{"name":"Sam","email":"s@x.io"}}],
            "invitations":[{"id":"i1","email":"new@x.io","status":"pending","expiresAt":"2030-01-01T00:00:00.000Z"},
                           {"id":"i2","email":"old@x.io","status":"pending","expiresAt":"2001-01-01T00:00:00.000Z"},
                           {"id":"i3","email":"done@x.io","status":"accepted","expiresAt":"2030-01-01T00:00:00.000Z"}]}""",
        )
        val orgs = j("""[{"id":"org1","name":"Home"},{"id":"org2","name":"Cabin"}]""")
        val h = AccountParse.hostedHousehold(org, orgs, me = "u2", now = 1_800_000_000)
        assertEquals("member", h.role)
        assertFalse(h.isOwner)
        assertEquals(listOf("Sam", "Al"), h.members.map { it.name })
        assertEquals(listOf(false, true), h.members.map { it.you })
        assertEquals(listOf("i1"), h.invites.map { it.id })
        assertEquals("new@x.io", h.invites.single().email)
        assertEquals("org2", h.others.single().id)
    }

    @Test
    fun devicesInEitherMode() {
        val accounts = AccountParse.accountsDevices(j("""[{"id":5,"userAgent":"Crumb/1 (Android 14; Pixel)","lastSeenAt":100,"current":true},{"id":6,"userAgent":null,"lastSeenAt":50,"current":false}]"""))
        assertEquals(listOf("5", "6"), accounts.map { it.id })
        assertTrue(accounts[0].current)
        assertNull(accounts[1].userAgent)

        val hosted = AccountParse.hostedDevices(
            j(
                """[{"id":"s1","token":"t1","userAgent":"a","updatedAt":"2026-01-02T00:00:00.000Z"},
                    {"id":"s2","token":"t2","userAgent":"b","updatedAt":"2026-01-03T00:00:00.000Z"},
                    {"id":"s3","token":"t3","userAgent":"c","updatedAt":"2026-01-01T00:00:00.000Z"}]""",
            ),
            currentSessionId = "s3",
        )
        // This device first, then the newest; ids are the revoke tokens
        assertEquals(listOf("t3", "t2", "t1"), hosted.map { it.id })
        assertTrue(hosted[0].current)
    }

    @Test
    fun signInMethodsInEitherMode() {
        val accounts = AccountParse.accountsMethods(j("""{"password":true,"linked":[{"provider":"google","email":"s@gmail.com","createdAt":10},{"provider":"github","createdAt":1}]}"""))
        assertTrue(accounts.password)
        assertEquals(listOf(Provider.Google), accounts.linked.map { it.provider })
        assertEquals("s@gmail.com", accounts.linked.single().email)

        val hosted = AccountParse.hostedMethods(j("""[{"id":"a1","providerId":"credential","createdAt":"2026-01-01T00:00:00.000Z"},{"id":"a2","providerId":"apple","createdAt":"2026-01-01T00:00:00.000Z"}]"""))
        assertTrue(hosted.password)
        assertEquals(listOf(Provider.Apple), hosted.linked.map { it.provider })
        assertEquals(1_767_225_600L, hosted.linked.single().createdAt)
    }
}
