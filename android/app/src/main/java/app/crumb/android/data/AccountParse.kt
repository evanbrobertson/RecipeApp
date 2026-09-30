package app.crumb.android.data

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.longOrNull
import java.time.Instant

// The two servers answer the same questions in different shapes (accounts: Rust's own
// numbers; hosted: Better Auth's organization plugin). These turn either into one model, as
// crumb-client's accounts.rs does. Pure, so they're unit-tested on sample bodies.

private fun JsonElement?.obj(): JsonObject? = this as? JsonObject
private fun JsonElement?.arr(): JsonArray = (this as? JsonArray) ?: JsonArray(emptyList())
private fun JsonElement?.text(): String? = (this as? JsonPrimitive)?.takeIf { it.isString || it.longOrNull != null }?.contentOrNull
private fun JsonObject.str(key: String): String = this[key].text().orEmpty()
private fun JsonObject.long(key: String): Long = (this[key] as? JsonPrimitive)?.longOrNull ?: 0L

/** Better Auth's ISO timestamps as unix seconds (0 when unreadable). */
fun isoSeconds(raw: String?): Long = runCatching { Instant.parse(raw!!).epochSecond }.getOrDefault(0L)

object AccountParse {
    /** Accounts' `GET /api/auth/household`. */
    fun accountsHousehold(root: JsonElement): Household {
        val h = root.jsonObject
        val you = h.long("you")
        return Household(
            id = h.str("id"),
            name = h.str("name"),
            role = h.str("role"),
            members = h["members"].arr().map {
                val m = it.jsonObject
                Member(m.str("userId"), m.str("name"), m.str("email"), m.str("role"), m.long("userId") == you)
            },
            invites = h["invites"].arr().map {
                val i = it.jsonObject
                PendingInvite(i.str("id"), null, i["createdBy"].text(), i.long("expiresAt"))
            },
            households = h["households"].arr().map { it.jsonObject.let { o -> HouseholdRef(o.str("id"), o.str("name")) } },
        )
    }

    /**
     * Hosted: `organization/get-full-organization` and `organization/list`, with the signed-in
     * user's id (from `get-session`) to find their role and mark them. Only pending, unexpired
     * invitations count; owners come first and no one else moves.
     */
    fun hostedHousehold(org: JsonElement, orgs: JsonElement, me: String?, now: Long): Household {
        val o = org.jsonObject
        val rows = o["members"].arr().map { it.jsonObject }
        val role = rows.firstOrNull { it.str("userId") == me }?.str("role") ?: "member"
        val members = rows.map { m ->
            val user = m["user"].obj()
            Member(m.str("id"), user?.str("name").orEmpty(), user?.str("email").orEmpty(), m.str("role"), m.str("userId") == me)
        }.sortedBy { it.role != "owner" }
        val invites = o["invitations"].arr().map { it.jsonObject }
            .filter { it.str("status") == "pending" && isoSeconds(it.str("expiresAt")) > now }
            .map { PendingInvite(it.str("id"), it.str("email"), null, isoSeconds(it.str("expiresAt"))) }
        return Household(
            id = o.str("id"),
            name = o.str("name"),
            role = role,
            members = members,
            invites = invites,
            households = orgs.arr().map { it.jsonObject.let { x -> HouseholdRef(x.str("id"), x.str("name")) } },
        )
    }

    /** Accounts' `GET /api/auth/sessions`. */
    fun accountsDevices(root: JsonElement): List<Device> = root.arr().map {
        val d = it.jsonObject
        Device(d.str("id"), d["userAgent"].text(), d.long("lastSeenAt"), (d["current"] as? JsonPrimitive)?.booleanOrNull == true)
    }

    /** Hosted `list-sessions`; [currentSessionId] is from `get-session`. This device first, then newest. */
    fun hostedDevices(root: JsonElement, currentSessionId: String?): List<Device> = root.arr().map {
        val d = it.jsonObject
        Device(d.str("token"), d["userAgent"].text(), isoSeconds(d.str("updatedAt")), d.str("id") == currentSessionId)
    }.sortedWith(compareByDescending<Device> { it.current }.thenByDescending { it.lastSeenAt })

    /** Accounts' `GET /api/auth/identities`. */
    fun accountsMethods(root: JsonElement): SignInMethods {
        val r = root.jsonObject
        return SignInMethods(
            password = (r["password"] as? JsonPrimitive)?.booleanOrNull == true,
            linked = r["linked"].arr().mapNotNull {
                val l = it.jsonObject
                Provider.of(l.str("provider"))?.let { p -> LinkedProvider(p, l["email"].text(), l.long("createdAt")) }
            },
        )
    }

    /** Hosted `list-accounts`: the `credential` row is the password. */
    fun hostedMethods(root: JsonElement): SignInMethods {
        val rows = root.arr().map { it.jsonObject }
        return SignInMethods(
            password = rows.any { it.str("providerId") == "credential" },
            linked = rows.mapNotNull {
                Provider.of(it.str("providerId"))?.let { p -> LinkedProvider(p, null, isoSeconds(it.str("createdAt"))) }
            },
        )
    }
}
