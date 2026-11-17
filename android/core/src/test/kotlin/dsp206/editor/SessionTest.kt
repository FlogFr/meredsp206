package dsp206.editor

import dsp206.link.CloseReason
import dsp206.link.Event
import dsp206.link.Health
import dsp206.proto.Bytes
import dsp206.proto.Channel
import dsp206.proto.Request
import dsp206.proto.factoryDefault
import dsp206.proto.apply
import dsp206.proto.linkRequests
import dsp206.proto.linkedChannels
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SessionTest {
    private var now = 1_000L
    private val session = Session { now }

    @Test
    fun readEventsFillTheModel() {
        session.connected(LinkKind.NETWORK)
        assertNull(session.handle(Event.Identity("DSP206 V0105P")))
        session.handle(Event.CurrentPreset(3))
        session.handle(Event.PresetName(3, "Club"))
        session.handle(Event.PresetName(21, "Current"))
        val p = factoryDefault().also { it.setGain(Channel.OUT1, 100) }
        session.handle(Event.Page(2, p.toPages()[2]))
        assertEquals(1, session.pagesReceived)
        assertEquals(UiNotice.ProgramRead, session.handle(Event.ProgramRead(p)))
        assertEquals("DSP206 V0105P", session.model.identity)
        assertEquals(3, session.model.currentPreset)
        assertEquals("Club", session.model.presetNames[3])
        assertEquals(p, session.model.program)
        session.handle(Event.PresetSlots(Bytes.zeros(20)))
        assertEquals(20, session.presetSlots?.size)
    }

    @Test
    fun problemsAreCountedAndReported() {
        session.connected(LinkKind.NETWORK)
        assertEquals(UiNotice.ProgramIncomplete(2), session.handle(Event.ProgramIncomplete(listOf(17, 18))))
        assertEquals(listOf(17, 18), session.missingPages)
        assertEquals(UiNotice.DeviceSilent, session.handle(Event.HealthChanged(Health.SILENT)))
        assertNull(session.handle(Event.HealthChanged(Health.ANSWERING)))
        session.handle(Event.Timeout(Request.Identify))
        assertNull(session.handle(Event.Unconfirmed(Request.SetMute(Channel.IN_A, true))))
        assertEquals(1 to 1, session.timeouts to session.unconfirmed)
        session.model.beginStore(4, "Live")
        assertEquals(
            UiNotice.Preset(Notice.StoreUnconfirmed(4)),
            session.handle(Event.Unconfirmed(Request.StorePreset(4))),
        )
    }

    @Test
    fun storeOutcomeComesFromTheNameReadBack() {
        session.model.beginStore(2, "Club")
        assertEquals(UiNotice.Preset(Notice.Stored(2, "Club")), session.handle(Event.PresetName(2, "Club")))
    }

    @Test
    fun networkCloseStartsTheReconnectCountdown() {
        session.connected(LinkKind.NETWORK)
        val notice = session.handle(Event.Closed(CloseReason.REMOTE_CLOSED))
        assertEquals(UiNotice.Disconnected(CloseReason.REMOTE_CLOSED, null), notice)
        assertEquals(LinkKind.OFFLINE, session.kind)
        assertEquals(60, session.reconnectWaitS())
        now += 59_500
        assertEquals(1, session.reconnectWaitS())
        now += 600
        assertNull(session.reconnectWaitS())
        // USB and demo sessions do not need the pause.
        session.connected(LinkKind.USB)
        session.closed()
        assertNull(session.reconnectWaitS())
    }

    @Test
    fun traceLinesNameTheCommand() {
        session.handle(Event.Trace(true, Bytes.hex("10 02 00 01 01 10 10 03 11")))
        session.handle(Event.Trace(false, Bytes.hex("10 02 01 00 01 01 10 03 00")))
        assertTrue(session.log[0].contains("→  10 02 00 01 01 10 10 03 11") && session.log[0].endsWith("connect"))
        assertTrue(session.log[1].endsWith("ack"))
        repeat(Session.LOG_LINES) { session.handle(Event.Trace(true, Bytes.of(1))) }
        assertEquals(Session.LOG_LINES, session.log.size)
        session.clearLog()
        assertTrue(session.log.isEmpty())
    }

    @Test
    fun editsGoThroughTheModel() {
        assertEquals(Request.SetGain(Channel.IN_A, 10), session.edit(Request.SetGain(Channel.IN_A, 10)))
        assertNull(session.edit(Request.SetGain(Channel.IN_A, 10)))
    }

    @Test
    fun parsesIpv4LiteralsOnly() {
        assertEquals("192.168.1.101", Ipv4.parse(" 192.168.1.101 ")?.hostAddress)
        assertNull(Ipv4.parse("192.168.1.256"))
        assertNull(Ipv4.parse("dsp.local"))
        assertNull(Ipv4.parse("192.168.1"))
        assertEquals(Ipv4.FACTORY_ADDRESS, Ipv4.parse(Ipv4.FACTORY_ADDRESS)?.hostAddress)
    }
}

class LinkGroupTest {
    @Test
    fun joiningAndLeavingAGroup() {
        val p = factoryDefault()
        val join = p.linkRequests(Channel.OUT1, Channel.OUT2, true)
        assertEquals(listOf(Request.SetLink(Channel.OUT1, 0b11), Request.SetLink(Channel.OUT2, 0b11)), join)
        join.forEach { p.apply(it) }
        assertEquals(listOf(Channel.OUT1, Channel.OUT2), p.linkedChannels(Channel.OUT2))
        val third = p.linkRequests(Channel.OUT2, Channel.OUT3, true)
        third.forEach { p.apply(it) }
        assertEquals(listOf(Channel.OUT1, Channel.OUT2, Channel.OUT3), p.linkedChannels(Channel.OUT1))
        val leave = p.linkRequests(Channel.OUT1, Channel.OUT2, false)
        assertEquals(
            listOf(Request.SetLink(Channel.OUT1, 0b101), Request.SetLink(Channel.OUT3, 0b101), Request.SetLink(Channel.OUT2, 0b10)),
            leave,
        )
        leave.forEach { p.apply(it) }
        assertEquals(listOf(Channel.OUT2), p.linkedChannels(Channel.OUT2))
        // Inputs and outputs never link together.
        assertTrue(p.linkRequests(Channel.IN_A, Channel.OUT1, true).isEmpty())
        assertTrue(p.linkRequests(Channel.IN_A, Channel.IN_A, true).isEmpty())
    }
}
