# 🆔 idSaveDataResigner-GUI – What is it ❓

A user-friendly GUI wrapper for idSaveDataResigner that allows you to **encrypt, decrypt, and resign SaveData files** from various games running on idTech Engine versions 7/8.

## Features
- **Main Tab**: Easy-to-use interface for Encrypt, Decrypt, and Resign operations.
- **Settings Tab**: Configure your desired output folder.
- **Batch Processing**: Processes all files in the selected folder.
- **Automatic Output Organization**: Creates folders with suffixes based on the operation mode.

## Supported titles
| Game Title                         | Platform Support          |
|------------------------------------|---------------------------|
| DOOM Eternal                       | Steam                     |
| Indiana Jones and the Great Circle | Steam **and GOG**         |
| DOOM The Dark Ages                 | Steam                     |

## Supported platforms
- Steam versions of all supported games  
- **GOG version supported for *Indiana Jones and the Great Circle* only**

# 🤔 Is it safe?
The short answer is: **No.**
> [!CAUTION]
> If you edit your SaveData files improperly, you risk corrupting them or getting banned from playing online. In both cases, you may lose your progress.

> [!IMPORTANT]
> Always create a backup of any files before editing them.

> [!IMPORTANT]
> Disable any cloud-sync feature (Steam Cloud, GOG Cloud Sync, etc.) before replacing SaveData files.

You've been warned. Now that you understand the possible consequences, you may proceed.

# 📜 How to use this tool

## Installation
1. Download the latest release from the releases page  
2. Place the executable in any folder  
3. Run `resigner.exe`

## Usage

### Main Tab
1. **Select Operation**: Encrypt, Decrypt, or Resign  
2. **Select Input Folder**: Choose the folder containing your SaveData files  
3. **Game Code**: Select the correct game code for your save files  
4. **User IDs**:
   - For **Decrypt/Encrypt**: Enter the original user ID associated with the save (platform-specific)  
   - For **Resign**: Enter both the original user ID and the target user ID  

   *(SteamID is only required for Steam saves. GOG saves do **not** require a SteamID.)*
5. **Process**: Click the process button to begin  

- An `INFO.txt` file is added inside the processed folder summarizing the performed action.
  
> [!TIP]
> You can use the SteamDB calculator at [steamdb.info](https://steamdb.info/calculator/) to find your 64-bit SteamID.

> [!TIP]
> After logging into [GOG](https://gog.com) you can open this url: `https://gog.com/userData.json` to access your userId or galaxyUserId (whichever is applicable) to be used.

> [!IMPORTANT]
> Don't share your IDs unless you're sure on what you're doing.

### Settings Tab
- **Output Folder**: Set your preferred output directory  
- Processed files are saved with the same input folder name plus a suffix describing the operation

### Important Notes
- You must know the **original user ID platform uses for the save** (SteamID for Steam games; not required for GOG saves)
- You may resign saves to any valid user ID for the selected platform
- Processed files are automatically organized with descriptive folder names

## Building from Source
```bash
# Clone the repository
git clone https://github.com/denuvosanctuary/idSaveDataResigner-GUI.git
cd idSaveDataResigner-GUI

# Build the project
cargo build --release

# Run the application
cargo run
```

# :heart: Credits
This GUI is built upon the excellent work of [mi5hmash](https://github.com/mi5hmash/) and their [idSaveDataResigner](https://github.com/mi5hmash/idSaveDataResigner/) project. All core logic and SaveData processing functionality is credited to their original implementation.
