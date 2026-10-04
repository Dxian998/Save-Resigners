# wdl-savecrypt

Rust CLI & library for decrypting, inspecting, and re-signing **Watch Dogs: Legion** save files (`.save`).

## CLI Usage

### 1. Inspect Save Info
Inspect container stamp, PRNG seed, payload size, and signature:
```bash
wdl-savecrypt inspect 1.save
```

### 2. Verify Account Binding
Check if a save file belongs to a specific Ubisoft Account UUID:
```bash
wdl-savecrypt verify 1.save --uuid 80f33a39-e682-4d1f-b693-39267e890df2
```

### 3. Decrypt Save
Export the raw Disrupt Engine serialized save data:
```bash
wdl-savecrypt decrypt 1.save -o 1.bin
```

### 4. Re-sign Save to Your Account
Take any save file and sign it directly to your target UUID:
```bash
wdl-savecrypt resign downloaded_1.save --uuid <YOUR_UBISOFT_UUID> -o 1.save
```

### 5. Batch Re-sign
Re-sign all `.save` files in a folder:
```bash
wdl-savecrypt batch-resign path/to/saves --uuid <YOUR_UBISOFT_UUID>
```

Or write re-signed files to a new directory:
```bash
wdl-savecrypt batch-resign path/to/source --uuid <YOUR_UBISOFT_UUID> --out-dir path/to/target
```

---
- go have a wonderful day
