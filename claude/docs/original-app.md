# The original Windows editor

*the t.racks DSP 206 Processor Editor V1.05*, installer
`435192_the_t.racks_dsp_206_processor_editor_v1.05_setup.exe`
(SHA-256 `49a10abe0395d6268f813157ae4a02cecb0125e4ed29dec618d1c1b10693fcc2`).

## 1. Packaging

- Inno Setup 5.2.3 installer with a single payload:
  `app/the t.racks DSP 206 Processor Editor V1.05.exe` (15.2 MB, PE32 i386,
  linked 2022-10-11).
- Built with Embarcadero C++Builder (VCL). Third-party VCL libraries: TMS
  (`TAdv*`, `TCurvyEdit`), Raize (`TRz*`), IOComp (`TiPlot`), JEDI
  (`TJvHidDeviceController`), Indy (`TIdUDPClient`), Borland ScktComp
  (`TClientSocket`).
- 40 forms are stored as binary DFM resources (`RT_RCDATA`, `TPF0`).

## 2. Forms

| Form | Purpose |
|---|---|
| `TfmMain` | Main window. All channel pages, matrix, meters, menus. |
| `TfmNetwork` | Scan and connect (COM / USB / NET), demo mode. |
| `TfmNetworkSetup` | Device ID (1–254) and IP (default 192.168.1.101) setup. |
| `TfmLinkSel` | Link groups for outputs. |
| `TfmCopy` | Channel copy. |
| `TfmStorePreset`, `TfmRecallPreset`, `TfmStoreName` | Presets. |
| `TfmChannelName`, `TfmChNameList` | Channel names. |
| `TfmLockSetup`, `TfmLockPassword`, `TfmPtsSetup`, `TfmPtsPassword` | Locks. |
| `TfmTest` | Test tone source (analog, pink, white, sine). |
| `TfmFactory` | Factory reset. |
| `TfmPeqRst`, `TfmExit`, `TfmInformation`, `TfmProcess`, `TfmAbout` | Dialogs. |
| `TfmRs232Code`, `TfmRs232CodeEnglish` | Help for the third-party RS-232 protocol. |
| `TfmHidData`, `TfmMemo` | Hidden debug views. |

## 3. Main menu (English string table)

File (Open preset `.prs`, Save preset, Upload unit to PC, Download unit from PC,
Factory Default, Exit) · Link · Copy · Lock · Setting ID/IP · Test Tone ·
Channel Name · Help (RS232 Code Table) · About.

## 4. Communication internals

- The transport thread class has one request slot, a queue with
  same-command coalescing, an 800 ms default timeout and a retry counter
  (`0x40be44` thread loop, `0x40c708` enqueue).
- Frames are built by `0x412d34` and parsed by `0x40d0dc` (byte state machine)
  then `0x412920` (frame → message).
- Received frames are posted to the main window as `WM_USER+0x150`. They are
  dispatched by `0x422bac`, a switch on the command byte.
- `TClientSocket` gets port 9761 and default IP octets `192.168.1.101` in
  `FormCreate` (`0x431b0f`).

## 5. Function address map (for further reverse engineering)

| Address | Role |
|---|---|
| `0x412d34` | Build frame (DLE/STX, XOR) |
| `0x422910` | Send message via the active link (COM/USB/NET) |
| `0x422070` | NET write (`ClientSocket->SendBuf`) |
| `0x45ce78` | `ClientSocketRead` |
| `0x422bac` | Reply dispatcher |
| `0x419cd8` | Parse 19 program pages into the program struct |
| `0x41aacc` | Parse stored-preset pages (upload flow) |
| `0x417a14` | Send a whole program (download flow) |
| `0x4214d0` / `0x421448` | Frequency index ↔ Hz |
| `0x425f18` | Level meter reply decoder |
| `0x403444` / `0x423730` / `0x416084` | Generic message with 0 / 1 / 2 data bytes |
| `0x413f3c` … `0x416590` | Typed message constructors (one per setter command) |

Published VCL event handlers and their addresses are listed by
`tools/re/methods.py` (348 names).
