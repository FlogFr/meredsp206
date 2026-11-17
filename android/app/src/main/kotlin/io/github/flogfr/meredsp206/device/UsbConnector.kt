package io.github.flogfr.meredsp206.device

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.os.Build
import androidx.core.content.ContextCompat
import dsp206.link.HidPort
import dsp206.proto.Hid
import java.io.IOException

/**
 * The DSP 206 over USB OTG (protocol.md §1.2): a HID device with one
 * interface, interrupt endpoints 0x81 IN and 0x02 OUT, 64-byte reports and no
 * report IDs.
 */
class UsbConnector(private val context: Context) {
    private val manager = context.getSystemService(UsbManager::class.java)

    /** The first DSP 206 plugged in, if any. */
    fun find(): UsbDevice? =
        manager?.deviceList?.values?.firstOrNull { it.vendorId == Hid.VENDOR_ID && it.productId == Hid.PRODUCT_ID }

    fun hasPermission(device: UsbDevice): Boolean = manager?.hasPermission(device) == true

    /** Ask the user for access to [device]; [onResult] runs on the main thread. */
    fun requestPermission(device: UsbDevice, onResult: (Boolean) -> Unit) {
        val m = manager ?: return onResult(false)
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(c: Context, intent: Intent) {
                context.unregisterReceiver(this)
                onResult(intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false))
            }
        }
        ContextCompat.registerReceiver(
            context,
            receiver,
            IntentFilter(ACTION_PERMISSION),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        // The system fills in the result extras, so the intent must be mutable
        // (and explicit, for Android 14 and later).
        val intent = Intent(ACTION_PERMISSION).setPackage(context.packageName)
        val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
        m.requestPermission(device, PendingIntent.getBroadcast(context, 0, intent, flags))
    }

    /** Open [device] and claim its HID interface (the kernel driver is detached). */
    fun open(device: UsbDevice): HidPort {
        val m = manager ?: throw IOException("USB host is not available")
        val intf = (0 until device.interfaceCount).map { device.getInterface(it) }
            .firstOrNull { it.interfaceClass == UsbConstants.USB_CLASS_HID }
            ?: throw IOException("no HID interface")
        val endpoints = (0 until intf.endpointCount).map { intf.getEndpoint(it) }
            .filter { it.type == UsbConstants.USB_ENDPOINT_XFER_INT }
        val input = endpoints.firstOrNull { it.direction == UsbConstants.USB_DIR_IN }
            ?: throw IOException("no interrupt IN endpoint")
        val output = endpoints.firstOrNull { it.direction == UsbConstants.USB_DIR_OUT }
        val conn = m.openDevice(device) ?: throw IOException("cannot open the USB device")
        if (!conn.claimInterface(intf, true)) {
            conn.close()
            throw IOException("cannot claim the USB interface")
        }
        return UsbHidPort(conn, intf, input, output)
    }

    companion object {
        private const val ACTION_PERMISSION = "io.github.flogfr.meredsp206.USB_PERMISSION"

        /** Broadcasts that tell the app a unit was plugged in or out. */
        fun attachFilter() = IntentFilter().apply {
            addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
            addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
        }

        fun isDsp206(intent: Intent): Boolean {
            val device = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra(UsbManager.EXTRA_DEVICE)
            }
            return device?.vendorId == Hid.VENDOR_ID && device.productId == Hid.PRODUCT_ID
        }
    }
}

/** 64-byte reports over the interrupt endpoints, or SET_REPORT when there is no OUT endpoint. */
private class UsbHidPort(
    private val conn: UsbDeviceConnection,
    private val intf: UsbInterface,
    private val input: UsbEndpoint,
    private val output: UsbEndpoint?,
) : HidPort {
    override fun write(report: ByteArray) {
        val n = if (output != null) {
            conn.bulkTransfer(output, report, report.size, WRITE_TIMEOUT_MS)
        } else {
            // HID class SET_REPORT, output report, ID 0.
            conn.controlTransfer(0x21, 0x09, 0x0200, intf.id, report, report.size, WRITE_TIMEOUT_MS)
        }
        if (n < 0) throw IOException("USB write failed")
    }

    override fun read(report: ByteArray, timeoutMs: Int): Int {
        val n = conn.bulkTransfer(input, report, report.size, timeoutMs)
        return if (n < 0) 0 else n
    }

    override fun close() {
        conn.releaseInterface(intf)
        conn.close()
    }

    private companion object {
        const val WRITE_TIMEOUT_MS = 1000
    }
}
