#pragma once
#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <span>
#include <string>
#include <vector>
namespace gta_asset_converter {
struct ConversionStats {
    std::size_t files_scanned = 0;
    std::size_t already_legacy = 0;
    std::size_t converted_ytd = 0;
    std::size_t converted_ydr = 0;
    std::size_t converted_ydd = 0;
    std::size_t converted_yft = 0;
    std::size_t converted_ypt = 0;
    std::size_t validated_ybn = 0;
    std::size_t rebuilt_ymap = 0;
    std::size_t rebuilt_ytyp = 0;
    std::size_t unchanged_other = 0;
    std::uint64_t bytes_scanned = 0;
};
std::vector<std::uint8_t> convert_asset_bytes(const std::string& extension, std::span<const std::uint8_t> input, ConversionStats& stats);
std::string convert_directory(const std::filesystem::path& root);
std::string inspect_rpf_keys(const std::filesystem::path& key_directory);
std::string extract_rpf_archive(const std::filesystem::path& source,
                                const std::filesystem::path& destination,
                                const std::filesystem::path& key_directory);
}
