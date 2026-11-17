package dsp206.editor

import dsp206.link.storePresetSequence
import dsp206.proto.CoalesceKey
import dsp206.proto.Names
import dsp206.proto.Program
import dsp206.proto.Request
import dsp206.proto.apply
import dsp206.proto.diffRequests
import dsp206.proto.factoryDefault

/** A preset operation outcome the UI tells the user about. */
sealed class Notice {
    data class Stored(val n: Int, val name: String) : Notice()

    /** The unit still names preset [n] [actual] after a store. */
    data class NotStored(val n: Int, val actual: String) : Notice()

    data class StoreUnconfirmed(val n: Int) : Notice()

    data class RecallUnconfirmed(val n: Int) : Notice()
}

/**
 * Editing state independent of the UI toolkit: the program, undo/redo and
 * device info. Port of crates/dsp206-gtk/src/model.rs. Not thread safe: use
 * it from one thread (the Android main thread).
 */
class EditorModel {
    var program: Program = factoryDefault()
        private set
    private val undo = ArrayDeque<Program>()
    private val redo = ArrayDeque<Program>()
    private var lastEdit: Pair<CoalesceKey, Long>? = null

    /** F00 then U01..U20. The unit does not report F00's name: it is the factory program. */
    val presetNames: MutableList<String> = MutableList(21) { "" }.also { it[0] = factoryDefault().nameText }
    var currentPreset: Int? = null
    var identity: String? = null

    /** A store waiting for its preset name to be read back: slot and name. */
    private var pendingStore: Pair<Int, String>? = null

    /** Apply a setter at time [nowMs]. Returns it when the program changed (to send it). */
    fun edit(req: Request, nowMs: Long): Request? {
        val before = program.copy()
        val next = program.copy()
        if (!next.apply(req)) return null
        program = next
        val key = req.coalesceKey
        val last = lastEdit
        val merge = key != null && last != null && last.first == key && nowMs - last.second < MERGE_WINDOW_MS
        if (!merge) {
            undo.addLast(before)
            if (undo.size > UNDO_LIMIT) undo.removeFirst()
        }
        lastEdit = key?.let { it to nowMs }
        redo.clear()
        return req
    }

    val canUndo: Boolean get() = undo.isNotEmpty()

    val canRedo: Boolean get() = redo.isNotEmpty()

    /** Go back one step; returns the setters that bring the device along. */
    fun undo(): List<Request> {
        val prev = undo.removeLastOrNull() ?: return emptyList()
        val reqs = program.diffRequests(prev)
        redo.addLast(program)
        program = prev
        lastEdit = null
        return reqs
    }

    fun redo(): List<Request> {
        val next = redo.removeLastOrNull() ?: return emptyList()
        val reqs = program.diffRequests(next)
        undo.addLast(program)
        program = next
        lastEdit = null
        return reqs
    }

    /** Replace the program with what the device holds (not undoable). */
    fun setFromDevice(p: Program) {
        program = p.copy()
        undo.clear()
        redo.clear()
        lastEdit = null
    }

    /** Overlay one received page on the current program. */
    fun applyPage(page: Int, data: dsp206.proto.Bytes) {
        val bytes = program.toBytes().copyOf(Program.PAGE_SIZE * Program.PAGE_COUNT)
        val start = page * Program.PAGE_SIZE
        if (page < 0 || start + data.size > bytes.size) return
        data.copyInto(bytes, start)
        program = Program.fromBytes(bytes)
    }

    /** Load a program from a file (undoable); returns the setters to send. */
    fun load(p: Program): List<Request> {
        val reqs = program.diffRequests(p)
        undo.addLast(program)
        program = p.copy()
        redo.clear()
        lastEdit = null
        return reqs
    }

    /**
     * Store the current program into user preset [n] (1 = U01). The program
     * takes the new name, as in the original editor. Returns the requests: the
     * store itself, then reads that let [storeOutcome] check it.
     */
    fun beginStore(n: Int, name: String): List<Request> {
        val next = program.copy()
        next.name = Names.presetField(name)
        program = next
        val text = Names.text(program.name)
        pendingStore = n to text
        return storePresetSequence(n, text) + listOf(Request.GetPresetName(n), Request.GetCurrentPreset)
    }

    /** Outcome for a preset name read back from the device, when it ends a pending store. */
    fun storeOutcome(index: Int, name: String): Notice? {
        val (n, expected) = pendingStore ?: return null
        if (n != index) return null
        pendingStore = null
        return if (name == expected) Notice.Stored(n, name) else Notice.NotStored(n, name)
    }

    /** Outcome for a preset operation the device did not acknowledge. */
    fun unconfirmed(req: Request): Notice? = when (req) {
        is Request.StorePreset -> pendingStore?.let { (n, _) ->
            pendingStore = null
            Notice.StoreUnconfirmed(n)
        }
        is Request.RecallPreset -> Notice.RecallUnconfirmed(req.n)
        else -> null
    }

    /** Record a preset name reported by the device (index 1..20; 21, the current program, is ignored). */
    fun setPresetName(index: Int, name: String) {
        if (index in presetNames.indices) presetNames[index] = name
    }

    companion object {
        /**
         * Edits to the same parameter within this window form one undo step
         * (dragging a fader produces one step, not hundreds).
         */
        const val MERGE_WINDOW_MS = 1200L
        const val UNDO_LIMIT = 200

        fun presetLabel(index: Int): String = if (index == 0) "F00" else "U%02d".format(index)
    }
}
