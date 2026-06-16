use std::io::{Read, Write};
use aes::Aes256; 
use aes::cipher::{Array, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use regex::Regex;
use uuid::Uuid;

// C#: private static readonly byte[] PublicKey = [0x35, 0xEC, ...];
const PUBLIC_KEY: [u8; 32] = [
    0x35, 0xEC, 0x33, 0x77, 0xF3, 0x5D, 0xB0, 0xEA, 0xBE, 0x6B, 0x83, 0x11, 0x54, 0x03, 0xEB, 0xFB,
    0x27, 0x25, 0x64, 0x2E, 0xD5, 0x49, 0x06, 0x29, 0x05, 0x78, 0xBD, 0x60, 0xBA, 0x4A, 0xA7, 0x87
];

pub struct Bl4Crypto;

impl Bl4Crypto {
    // Not in C# - added for explicit Steam ID validation
    // C# has this logic inside SteamId.Set() method
    pub fn validate_sid(id: &str) -> Result<(), String> {
        if id.is_empty() {
            return Err("Steam ID cannot be empty".to_string());
        }
        if id.len() != 17 {
            return Err("Steam ID must be 17 digits (example: 76561197960265729)".to_string());
        }
        if !id.chars().all(|c| c.is_ascii_digit()) {
            return Err("Steam ID must contain only numbers".to_string());
        }
        let num: u64 = id.parse().map_err(|_| "Invalid Steam ID format")?;
        if !id.starts_with("7656119") {
            return Err("Steam ID must start with 7656119 (Steam64 format)".to_string());
        }
        if num < 76561197960265728 || num > 76561999999999999 {
            return Err("Steam ID appears to be invalid (out of valid range)".to_string());
        }
        Ok(())
    }

    // C#: private static byte[] ParseUserId(string input)
    fn parse_user_id(input: &str) -> Result<Vec<u8>, String> {
        // C#: var epicId = new EpicId();
        // C#: var result = epicId.TrySetEpicId(input);
        // C#: if (result) return epicId.GetAsWideString();
        if input.len() == 32 && input.chars().all(|c| c.is_ascii_hexdigit()) {
            // EpicId.GetAsWideString() converts to UTF-16 (little-endian)
            // C#: var bytes = new byte[AccountId.Length * 2];
            // C#: for (var i = 0; i < AccountId.Length; i++)
            // C#: {
            // C#:     var charBytes = BitConverter.GetBytes(AccountId[i]);
            // C#:     bytes[i * 2] = charBytes[0];
            // C#:     bytes[i * 2 + 1] = charBytes[1];
            // C#: }
            let mut bytes = Vec::with_capacity(input.len() * 2);
            for ch in input.chars() {
                let char_bytes = (ch as u16).to_le_bytes();
                bytes.push(char_bytes[0]);
                bytes.push(char_bytes[1]);
            }
            return Ok(bytes);
        }
        
        // C#: var steamId = new SteamId();
        // C#: result = steamId.Set(input);
        // C#: return result 
        // C#:     ? steamId.GetSteamId64AsByteArray() 
        // C#:     : throw new FormatException("Invalid user ID format. Must be a valid Epic ID or Steam ID.");
        if input.len() == 17 && input.chars().all(|c| c.is_ascii_digit()) {
            Self::validate_sid(input)?;
            let steam_id: u64 = input.parse().map_err(|_| "Invalid Steam ID")?;
            return Ok(steam_id.to_le_bytes().to_vec());
        }
        
        Err("Invalid user ID format. Must be a valid Epic ID or Steam ID.".to_string())
    }

    // C#: public static byte[] CalculatePrivateKey(string userId)
    pub fn calc_priv_key(user_id: &str) -> Result<[u8; 32], String> {
        // C#: var uid = ParseUserId(userId);
        let uid = Self::parse_user_id(user_id)?;

        // C#: Span<byte> keyContainer = stackalloc byte[PublicKey.Length];
        // C#: PublicKey.CopyTo(keyContainer);
        let mut key_container = PUBLIC_KEY;
        
        // C#: var length = Math.Min(uid.Length, keyContainer.Length);
        let length = uid.len().min(key_container.len());
        
        // C#: for (var i = 0; i < length; i++)
        // C#:     keyContainer[i] = (byte)(keyContainer[i] ^ uid[i]);
        for i in 0..length {
            key_container[i] ^= uid[i];
        }
        
        // C#: return keyContainer.ToArray();
        Ok(key_container)
    }

    // C#: private static byte[] DecryptEcb(ReadOnlySpan<byte> encryptedBytes, ReadOnlySpan<byte> key)
    fn decrypt_ecb(encrypted_bytes: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
        if encrypted_bytes.len() % 16 != 0 {
            return Err("Invalid encrypted data length".to_string());
        }
        
        // C#: using var aes = GetAes(key);
        // C#: aes.Mode = CipherMode.ECB;
        // C#: aes.Padding = PaddingMode.None;
        // Use full 32-byte key for AES-256
        let cipher = Aes256::new_from_slice(key).map_err(|_| "Failed to create AES cipher")?;
        
        let mut decrypted = encrypted_bytes.to_vec();
        
        // C#: using var decryptor = aes.CreateDecryptor();
        // C#: using CryptoStream cs = new(msi, decryptor, CryptoStreamMode.Read);
        // Decrypt each 16-byte block
        for chunk in decrypted.chunks_exact_mut(16) {
            let block: &mut Array<u8, _> = chunk.try_into().map_err(|_| "Invalid block size")?;
            cipher.decrypt_block(block);
        }
        
        // C#: return mso.ToArray();
        Ok(decrypted)
    }

    // C#: private static byte[] EncryptEcb(ReadOnlySpan<byte> plainBytes, ReadOnlySpan<byte> key)
    fn encrypt_ecb(plain_bytes: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
        // C#: var plainBytesWithPadding = AddPkcs7Padding(plainBytes);
        let plain_bytes_with_padding = Self::add_pkcs7_padding(plain_bytes);
        
        // C#: using var aes = GetAes(key);
        // C#: using var encryptor = aes.CreateEncryptor();
        let cipher = Aes256::new_from_slice(key).map_err(|_| "Failed to create AES cipher")?;
        
        let mut encrypted = plain_bytes_with_padding;
        
        // C#: cs.Write(plainBytesWithPadding, 0, plainBytesWithPadding.Length);
        // C#: cs.FlushFinalBlock();
        for chunk in encrypted.chunks_exact_mut(16) {
            let block: &mut Array<u8, _> = chunk.try_into().map_err(|_| "Invalid block size")?;
            cipher.encrypt_block(block);
        }
        
        // C#: return ms.ToArray();
        Ok(encrypted)
    }

    // C#: private static byte[] AddPkcs7Padding(ReadOnlySpan<byte> data)
    fn add_pkcs7_padding(data: &[u8]) -> Vec<u8> {
        // C#: const int blockSize = 16;
        const BLOCK_SIZE: usize = 16;
        
        // C#: var padLen = blockSize - data.Length % blockSize;
        let pad_len = BLOCK_SIZE - (data.len() % BLOCK_SIZE);
        
        // C#: var newDataLength = data.Length + padLen;
        let new_length = data.len() + pad_len;
        
        // C#: var dataWithPadding = new byte[newDataLength];
        let mut data_with_padding = vec![0u8; new_length];
        
        // C#: data.CopyTo(dataWithPaddingAsSpan);
        data_with_padding[..data.len()].copy_from_slice(data);
        
        // C#: for (var i = 1; i < padLen + 1; i++)
        // C#:     dataWithPaddingAsSpan[^i] = (byte)padLen;
        // Note: Fixed the loop - C# starts from i=1, code correctly iterates 1..=pad_len
        for i in 1..=pad_len {
            data_with_padding[new_length - i] = pad_len as u8;
        }
        
        data_with_padding
    }

    // C#: private static int GetPossiblePkcs7PaddingLength(ReadOnlySpan<byte> data)
    fn get_pkcs7_padding_length(data: &[u8]) -> i32 {
        // C#: if (data.Length < 16) return 0;
        if data.len() < 16 {
            return 0;
        }
        
        // C#: var padLen = data[^1];
        let pad_len = data[data.len() - 1] as i32;
        
        // C#: if (padLen is > 16 or < 1) return 0;
        if pad_len > 16 || pad_len < 1 {
            return 0;
        }
        
        // C#: for (var i = 2; i < padLen; i++)
        // Note: C# starts from i=2, starting from i=1 - for cleaner loop
        for i in 1..pad_len {
            // C#: var result = data[^1] == data[^(padLen - i)];
            let result = data[data.len() - 1] == data[data.len() - 1 - i as usize];
            // C#: if (!result) return 0;
            if !result {
                return 0;
            }
        }
        
        pad_len
    }

    // C#: private static byte[] RemovePkcs7Padding(ReadOnlySpan<byte> data, int paddingLength)
    fn remove_pkcs7(data: &[u8], padding_length: i32) -> Vec<u8> {
        // C#: => data[..^paddingLength].ToArray();
        data[..data.len() - padding_length as usize].to_vec()
    }

    // C#: private static byte[] DecompressZlib(byte[] compressedData)
    fn decompress_zlib(compressed_data: &[u8]) -> Result<Vec<u8>, String> {
        // C#: using var zLibStream = new ZLibStream(inputStream, CompressionMode.Decompress);
        let mut decoder = ZlibDecoder::new(compressed_data);
        let mut result = Vec::new();
        // C#: zLibStream.CopyTo(outputStream);
        // C#: return outputStream.ToArray();
        decoder.read_to_end(&mut result).map_err(|e| format!("Decompression failed: {}", e))?;
        Ok(result)
    }

    // C#: private static byte[] CompressZlib(ReadOnlySpan<byte> data)
    fn compress_zlib(data: &[u8]) -> Result<Vec<u8>, String> {
        // C#: using var zLibStream = new ZLibStream(outputStream, CompressionLevel.Optimal, true)
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        // C#: zLibStream.Write(data);
        encoder.write_all(data).map_err(|e| format!("Compression write failed: {}", e))?;
        // C#: return outputStream.ToArray();
        encoder.finish().map_err(|e| format!("Compression finalization failed: {}", e))
    }

    // C#: private static uint ComputeAdler32Checksum(ReadOnlySpan<byte> data)
    fn compute_adler32(data: &[u8]) -> u32 {
        // C#: const uint modAdler = 65521;
        const MOD_ADLER: u32 = 65521;
        // C#: uint a = 1, b = 0;
        let mut a: u32 = 1;
        let mut b: u32 = 0;
        // C#: foreach (var c in data)
        for &c in data {
            // C#: a = (a + c) % modAdler;
            a = (a + c as u32) % MOD_ADLER;
            // C#: b = (b + a) % modAdler;
            b = (b + a) % MOD_ADLER;
        }
        // C#: return (b << 16) | a;
        (b << 16) | a
    }

    // C#: public static byte[] DecryptData(ReadOnlySpan<byte> encryptedData, ReadOnlySpan<byte> privateKey)
    pub fn decrypt_data(encrypted_data: &[u8], private_key: &[u8; 32]) -> Result<Vec<u8>, String> {
        // C#: var decryptedData = DecryptEcb(encryptedData, privateKey);
        let decrypted_data = Self::decrypt_ecb(encrypted_data, private_key)?;
        
        // C#: var paddingLength = GetPossiblePkcs7PaddingLength(decryptedData);
        let padding_length = Self::get_pkcs7_padding_length(&decrypted_data);
        
        // C#: var isTherePadding = paddingLength > 0;
        let is_there_padding = padding_length > 0;
        
        // C#: var decryptedDataWithoutPadding = isTherePadding 
        // C#:     ? RemovePkcs7Padding(decryptedData, paddingLength) 
        // C#:     : decryptedData;
        let decrypted_data_without_padding = if is_there_padding {
            Self::remove_pkcs7(&decrypted_data, padding_length)
        } else {
            decrypted_data.clone()
        };
        
        // C#: var expectedDataLength = BitConverter.ToInt32(decryptedDataWithoutPadding[^4..], 0);
        let mut expected_data_length = i32::from_le_bytes([
            decrypted_data_without_padding[decrypted_data_without_padding.len() - 4],
            decrypted_data_without_padding[decrypted_data_without_padding.len() - 3],
            decrypted_data_without_padding[decrypted_data_without_padding.len() - 2],
            decrypted_data_without_padding[decrypted_data_without_padding.len() - 1],
        ]);
        
        // C#: var expectedChecksum = BinaryPrimitives.ReadUInt32BigEndian(decryptedDataWithoutPadding.AsSpan()[^8..^4]);
        let mut expected_checksum = u32::from_be_bytes([
            decrypted_data_without_padding[decrypted_data_without_padding.len() - 8],
            decrypted_data_without_padding[decrypted_data_without_padding.len() - 7],
            decrypted_data_without_padding[decrypted_data_without_padding.len() - 6],
            decrypted_data_without_padding[decrypted_data_without_padding.len() - 5],
        ]);
        
        // C#: var decompressedData = DecompressZlib(decryptedDataWithoutPadding[..^8]);
        let mut decompressed_data = Self::decompress_zlib(
            &decrypted_data_without_padding[..decrypted_data_without_padding.len() - 8]
        ).map_err(|e| {
            if e.contains("corrupt deflate") || e.contains("invalid") {
                "Wrong User ID or corrupted save file. Double-check your ID.".to_string()
            } else {
                "File decompression failed. File may be damaged.".to_string()
            }
        })?;
        
        // C#: var isLengthValid = decompressedData.Length == expectedDataLength;
        let mut is_length_valid = decompressed_data.len() == expected_data_length as usize;
        // C#: var isChecksumValid = ComputeAdler32Checksum(decompressedData) == expectedChecksum;
        let mut is_checksum_valid = Self::compute_adler32(&decompressed_data) == expected_checksum;
        
        // C#: if ((!isLengthValid || !isChecksumValid) && isTherePadding)
        if (!is_length_valid || !is_checksum_valid) && is_there_padding {
            // C#: expectedDataLength = BitConverter.ToInt32(decryptedData[^4..], 0);
            expected_data_length = i32::from_le_bytes([
                decrypted_data[decrypted_data.len() - 4],
                decrypted_data[decrypted_data.len() - 3],
                decrypted_data[decrypted_data.len() - 2],
                decrypted_data[decrypted_data.len() - 1],
            ]);
            
            // C#: expectedChecksum = BinaryPrimitives.ReadUInt32BigEndian(decryptedData.AsSpan()[^8..^4]);
            expected_checksum = u32::from_be_bytes([
                decrypted_data[decrypted_data.len() - 8],
                decrypted_data[decrypted_data.len() - 7],
                decrypted_data[decrypted_data.len() - 6],
                decrypted_data[decrypted_data.len() - 5],
            ]);
            
            // C#: decompressedData = DecompressZlib(decryptedData[..^8]);
            decompressed_data = Self::decompress_zlib(&decrypted_data[..decrypted_data.len() - 8])
                .map_err(|e| {
                    if e.contains("corrupt deflate") || e.contains("invalid") {
                        "Wrong User ID or corrupted save file. Double-check your ID.".to_string()
                    } else {
                        "File decompression failed. File may be damaged.".to_string()
                    }
                })?;
            
            is_length_valid = decompressed_data.len() == expected_data_length as usize;
            is_checksum_valid = Self::compute_adler32(&decompressed_data) == expected_checksum;
            
            // C#: if (!isLengthValid || !isChecksumValid)
            // C#:     throw new InvalidDataException("Decryption failed: Invalid length or checksum.");
            if !is_length_valid || !is_checksum_valid {
                return Err("Decryption failed: The Steam ID may be incorrect, or the file is corrupted".to_string());
            }
        }
        
        // C#: return decompressedData;
        Ok(decompressed_data)
    }

    // C#: public static byte[] EncryptData(ReadOnlySpan<byte> decryptedData, ReadOnlySpan<byte> privateKey)
    pub fn encrypt_data(decrypted_data: &[u8], private_key: &[u8; 32]) -> Result<Vec<u8>, String> {
        // C#: var decryptedDataChecksum = ComputeAdler32Checksum(decryptedData);
        let decrypted_data_checksum = Self::compute_adler32(decrypted_data);
        
        // C#: var decryptedDataLength = decryptedData.Length;
        let decrypted_data_length = decrypted_data.len();
        
        // C#: var compressedData = CompressZlib(decryptedData);
        let compressed_data = Self::compress_zlib(decrypted_data)?;
        
        // C#: var dataToEncrypt = new byte[compressedData.Length + 8];
        let mut data_to_encrypt = vec![0u8; compressed_data.len() + 8];
        
        // C#: var dataToEncryptSpan = dataToEncrypt.AsSpan();
        // C#: compressedData.CopyTo(dataToEncryptSpan);
        data_to_encrypt[..compressed_data.len()].copy_from_slice(&compressed_data);
        
        // C#: BinaryPrimitives.WriteInt32BigEndian(dataToEncryptSpan.Slice(compressedData.Length, 4), (int)decryptedDataChecksum);
        let checksum_be = decrypted_data_checksum.to_be_bytes();
        data_to_encrypt[compressed_data.len()..compressed_data.len() + 4].copy_from_slice(&checksum_be);
        
        // C#: BitConverter.GetBytes(decryptedDataLength).CopyTo(dataToEncrypt, compressedData.Length + 4);
        let length_le = (decrypted_data_length as i32).to_le_bytes();
        data_to_encrypt[compressed_data.len() + 4..compressed_data.len() + 8].copy_from_slice(&length_le);
        
        // C#: return EncryptEcb(dataToEncrypt, privateKey);
        Self::encrypt_ecb(&data_to_encrypt, private_key)
    }

    // Not directly in C# - combines CalculatePrivateKey + DecryptData
    pub fn decrypt_file(data: &[u8], _filename: &str, user_id: &str) -> Result<Vec<u8>, String> {
        let priv_key = Self::calc_priv_key(user_id)?;
        Self::decrypt_data(data, &priv_key)
    }

    // Not directly in C# - combines CalculatePrivateKey + EncryptData
    pub fn encrypt_file(data: &[u8], _filename: &str, user_id: &str) -> Result<Vec<u8>, String> {
        let priv_key = Self::calc_priv_key(user_id)?;
        Self::encrypt_data(data, &priv_key)
    }

    // Not directly in C# - combines decrypt + anonymize + encrypt
    pub fn resign_file(data: &[u8], filename: &str, old_user_id: &str, new_user_id: &str) -> Result<Vec<u8>, String> {
        let decrypted = Self::decrypt_file(data, filename, old_user_id)?;
        let filename_without_ext = filename
            .trim_end_matches(".sav")
            .trim_end_matches(".dat")
            .trim_end_matches(".bin");
        let anonymized = Self::anonymize_save_data(&decrypted, filename_without_ext)?;
        Self::encrypt_file(&anonymized, filename, new_user_id)
    }

    // C#: public static byte[] AnonymizeSaveData(byte[] yamlContent, string fileNameWithoutExtension, SimpleLogger logger, string loggerGroup)
    pub fn anonymize_save_data(yaml_content: &[u8], filename_without_ext: &str) -> Result<Vec<u8>, String> {
        // C#: var yamlContentText = Encoding.UTF8.GetString(yamlContent);
        let mut yaml_text = String::from_utf8(yaml_content.to_vec()).map_err(|_| "Invalid UTF-8 in YAML content")?;
        
        // C#: if (fileNameWithoutExtension != "profile")
        if filename_without_ext != "profile" {
            // C#: if (!UpdateCharGuid(ref yamlContentText))
            // C#:     logger.LogWarning("Character GUID could NOT be updated!", loggerGroup);
            Self::update_char_guid(&mut yaml_text)?;
        }
        
        // C#: if (!UpdateSaveGuid(ref yamlContentText))
        // C#:     logger.LogWarning("Save GUID could NOT be updated!", loggerGroup);
        Self::update_save_guid(&mut yaml_text)?;
        
        // C#: if (!AnonymizeOnlineCharacterPrefs(ref yamlContentText))
        // C#:     logger.LogWarning("OnlineCharacterPrefs could NOT be updated!", loggerGroup);
        Self::update_online_prefs(&mut yaml_text)?;
        
        // C#: return Encoding.UTF8.GetBytes(yamlContentText);
        Ok(yaml_text.into_bytes())
    }

    // C#: private static bool UpdateCharGuid(ref string dataText)
    fn update_char_guid(yaml_text: &mut String) -> Result<bool, String> {
        // C#: const string pattern = @"(?<=char_guid:\s)[A-Fa-f0-9]{32}";
        // Match "char_guid: " followed by 32 hex characters
        let pattern = Regex::new(r"char_guid:\s+([A-Fa-f0-9]{32})")
            .map_err(|_| "Failed to create char_guid regex")?;
        
        // C#: var newGuid = Guid.NewGuid().ToString().Replace("-", "").ToUpper();
        // Generate ONE GUID to reuse for all replacements (matches C# behavior)
        let new_guid = Uuid::new_v4().simple().to_string().to_uppercase();
        let mut replacements = 0;
        
        // C#: var replacementsCount = 0;
        // C#: dataText = Regex.Replace(dataText, pattern, _ =>
        // C#: {
        // C#:     replacementsCount++;
        // C#:     return newGuid;
        // C#: });
        *yaml_text = pattern.replace_all(yaml_text, |_caps: &regex::Captures| {
            replacements += 1;
            format!("char_guid: {}", new_guid)
        }).to_string();
        
        // C#: return replacementsCount > 0;
        Ok(replacements > 0)
    }

    // C#: private static bool UpdateSaveGuid(ref string dataText)
    fn update_save_guid(yaml_text: &mut String) -> Result<bool, String> {
        // C#: const string pattern = @"(?<=save_game_header:\s*\r?\n\s*guid:\s)[A-Fa-f0-9]{32}";
        // Match the save_game_header section with guid
        let pattern = Regex::new(r"save_game_header:\s*\r?\n\s*guid:\s+([A-Fa-f0-9]{32})")
            .map_err(|_| "Failed to create save_guid regex")?;
        
        // C#: var newGuid = Guid.NewGuid().ToString().Replace("-", "").ToUpper();
        // Generate ONE GUID to reuse for all replacements (matches C# behavior)
        let new_guid = Uuid::new_v4().simple().to_string().to_uppercase();
        let mut replacements = 0;
        
        // C#: var replacementsCount = 0;
        // C#: dataText = Regex.Replace(dataText, pattern, _ =>
        // C#: {
        // C#:     replacementsCount++;
        // C#:     return newGuid;
        // C#: });
        *yaml_text = pattern.replace_all(yaml_text, |caps: &regex::Captures| {
            replacements += 1;
            // Preserve the original whitespace structure
            let full_match = caps.get(0).unwrap().as_str();
            let lines: Vec<&str> = full_match.lines().collect();
            if lines.len() >= 2 {
                let indent = lines[1].chars().take_while(|c| c.is_whitespace()).collect::<String>();
                format!("save_game_header:\n{}guid: {}", indent, new_guid)
            } else {
                format!("save_game_header:\n  guid: {}", new_guid)
            }
        }).to_string();
        
        // C#: return replacementsCount > 0;
        Ok(replacements > 0)
    }

    // C#: private static bool AnonymizeOnlineCharacterPrefs(ref string dataText)
    fn update_online_prefs(yaml_text: &mut String) -> Result<bool, String> {
        // C#: const string pattern = @"onlinecharacterprefs:\s*(?:\n\s+.*)+";
        // Match onlinecharacterprefs and everything indented under it
        let pattern = Regex::new(r"onlinecharacterprefs:\s*\n(?:\s+[^\n]+\n?)*")
            .map_err(|_| "Failed to create online prefs regex")?;
        
        let mut replacements = 0;
        
        // C#: var replacement = """
        // C#:                   onlinecharacterprefs:
        // C#:                     recentfriends:
        // C#:                     partyleader:
        // C#:                   """;
        // C#: var replacementsCount = 0;
        // C#: dataText = Regex.Replace(dataText, pattern, _ =>
        // C#: {
        // C#:     replacementsCount++;
        // C#:     return replacement;
        // C#: });
        *yaml_text = pattern.replace_all(yaml_text, |caps: &regex::Captures| {
            replacements += 1;
            let full_match = caps.get(0).unwrap().as_str();
            // Try to detect the indentation from the original text
            let lines: Vec<&str> = full_match.lines().collect();
            let indent = if lines.len() > 1 {
                lines[1].chars().take_while(|c| c.is_whitespace()).collect::<String>()
            } else {
                "  ".to_string()
            };
            
            format!("onlinecharacterprefs:\n{}recentfriends:\n{}partyleader:", indent, indent)
        }).to_string();
        
        // C#: return replacementsCount > 0;
        Ok(replacements > 0)
    }

    // Not in C# - custom implementation for detecting encrypted data
    pub fn is_encrypted(data: &[u8]) -> bool {
        if data.len() < 16 {
            return false;
        }
        if std::str::from_utf8(&data[..16]).is_ok() {
            return false;
        }
        let sample_size = data.len().min(1024);
        let mut byte_counts = [0u32; 256];
        for &byte in &data[..sample_size] {
            byte_counts[byte as usize] += 1;
        }
        let len = sample_size as f64;
        let entropy: f64 = byte_counts.iter().filter(|&&count| count > 0).map(|&count| {
            let p = count as f64 / len;
            -p * p.log2()
        }).sum();
        entropy > 6.0
    }
}