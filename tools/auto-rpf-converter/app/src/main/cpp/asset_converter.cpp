#include "asset_converter.h"
#include <algorithm>
#include <cctype>
#include <fstream>
#include <limits>
#include <stdexcept>
#include <system_error>
#include "ragekit/ragekit.h"
namespace gta_asset_converter {
namespace fs = std::filesystem;
namespace {
constexpr std::uint64_t kMaxAssetBytes = 512ull * 1024ull * 1024ull;
constexpr std::uint64_t kMaxTotalBytes = 8ull * 1024ull * 1024ull * 1024ull;
constexpr std::size_t kMaxFiles = 1'000'000;
std::string lower(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return value;
}
bool supported_extension(const std::string& ext) {
    return ext == "ytd" || ext == "ydr" || ext == "ydd" || ext == "yft" || ext == "ypt" || ext == "ybn" || ext == "ymap" || ext == "ytyp";
}
int legacy_version(const std::string& ext) {
    if (ext == "ytd") return 13;
    if (ext == "ydr" || ext == "ydd") return 165;
    if (ext == "yft") return 162;
    if (ext == "ypt") return 68;
    return 0;
}
int gen9_version(const std::string& ext) {
    if (ext == "ytd") return 5;
    if (ext == "ydr" || ext == "ydd") return 159;
    if (ext == "yft") return 171;
    if (ext == "ypt") return 71;
    return 0;
}
void write_replacement(const fs::path& path, std::span<const std::uint8_t> bytes) {
    fs::path temporary = path;
    temporary += ".legacy-convert.tmp";
    std::error_code ignored;
    fs::remove(temporary, ignored);
    {
        std::ofstream out(temporary, std::ios::binary | std::ios::trunc);
        if (!out) throw std::runtime_error("cannot create converted temporary file: " + temporary.string());
        out.write(reinterpret_cast<const char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
        out.flush();
        if (!out) {
            fs::remove(temporary, ignored);
            throw std::runtime_error("cannot write converted temporary file: " + temporary.string());
        }
    }
    std::error_code ec;
    fs::rename(temporary, path, ec);
    if (ec) {
        fs::remove(temporary, ignored);
        throw std::runtime_error("cannot replace converted asset " + path.string() + ": " + ec.message());
    }
}
}
std::vector<std::uint8_t> convert_asset_bytes(const std::string& extension, std::span<const std::uint8_t> input, ConversionStats& stats) {
    const std::string ext = lower(extension);
    if (!supported_extension(ext)) {
        ++stats.unchanged_other;
        return {input.begin(), input.end()};
    }
    if (input.size() > kMaxAssetBytes) throw std::runtime_error(ext + " exceeds the 512 MiB per-asset safety limit");
    const auto resource = rk::rsc::Resource::load(input);
    // Collision and map resources do not have a Gen9-specific layout: GTA V Legacy and
    // Enhanced both use YBN=43 and YMAP/YTYP=2. Parse and serialize them through ragekit
    // so malformed data is rejected and the semantic structures are rebuilt, not renamed.
    if (ext == "ybn") {
        if (resource.version != 43) throw std::runtime_error("YBN has unsupported RSC7 resource version " + std::to_string(resource.version));
        auto bound = rk::read_ybn(input);
        auto output = rk::write_ybn(*bound);
        ++stats.validated_ybn;
        return output;
    }
    if (ext == "ymap") {
        if (resource.version != 2) throw std::runtime_error("YMAP has unsupported RSC7 resource version " + std::to_string(resource.version));
        auto map = rk::read_ymap(input);
        auto output = rk::write_ymap(map);
        ++stats.rebuilt_ymap;
        return output;
    }
    if (ext == "ytyp") {
        if (resource.version != 2) throw std::runtime_error("YTYP has unsupported RSC7 resource version " + std::to_string(resource.version));
        auto types = rk::read_ytyp(input);
        auto output = rk::write_ytyp(types);
        ++stats.rebuilt_ytyp;
        return output;
    }
    if (resource.version == legacy_version(ext)) {
        ++stats.already_legacy;
        return {input.begin(), input.end()};
    }
    const bool is_gen9 = resource.version == gen9_version(ext) ||
        ((ext == "ydr" || ext == "ydd") && resource.version == 154);
    if (!is_gen9) {
        throw std::runtime_error(ext + " has unsupported RSC7 resource version " + std::to_string(resource.version));
    }
    if (ext == "ytd") {
        auto dictionary = rk::read_ytd(input);
        auto output = rk::write_ytd(dictionary, rk::Game::Legacy);
        ++stats.converted_ytd;
        return output;
    }
    if (ext == "ydr") {
        auto drawable = rk::read_ydr(input);
        auto output = rk::write_ydr(drawable, rk::Game::Legacy);
        ++stats.converted_ydr;
        return output;
    }
    if (ext == "ydd") {
        auto dictionary = rk::read_ydd(input);
        auto output = rk::write_ydd(dictionary, rk::Game::Legacy);
        ++stats.converted_ydd;
        return output;
    }
    if (ext == "yft") {
        auto fragment = rk::read_yft(input);
        auto output = rk::write_yft(fragment, rk::Game::Legacy);
        ++stats.converted_yft;
        return output;
    }
    if (ext == "ypt") {
        auto particles = rk::read_ypt(input);
        auto output = rk::write_ypt(particles, rk::Game::Legacy);
        ++stats.converted_ypt;
        return output;
    }
    throw std::runtime_error("no conversion handler for " + ext);
}
std::string convert_directory(const fs::path& root) {
    if (!fs::exists(root) || !fs::is_directory(root)) throw std::runtime_error("asset conversion input is not a directory");
    ConversionStats stats;
    for (fs::recursive_directory_iterator it(root), end; it != end; ++it) {
        const auto status = it->symlink_status();
        if (fs::is_symlink(status)) throw std::runtime_error("symbolic links are not allowed in conversion input: " + it->path().string());
        if (!fs::is_regular_file(status)) continue;
        ++stats.files_scanned;
        if (stats.files_scanned > kMaxFiles) throw std::runtime_error("asset count exceeds the one-million-file safety limit");
        const auto size = fs::file_size(it->path());
        const std::string ext = lower(it->path().extension().string());
        const bool is_supported = ext.size() > 1 && ext[0] == '.' && supported_extension(ext.substr(1));
        if (size > kMaxAssetBytes && is_supported) throw std::runtime_error("supported resource exceeds the 512 MiB per-asset safety limit: " + it->path().string());
        if (size > std::numeric_limits<std::uint64_t>::max() - stats.bytes_scanned) throw std::runtime_error("asset byte count overflow");
        stats.bytes_scanned += size;
        if (stats.bytes_scanned > kMaxTotalBytes) throw std::runtime_error("asset tree exceeds the 8 GiB safety limit");
        if (!is_supported) {
            ++stats.unchanged_other;
            continue;
        }
        const auto input = rk::rsc::read_file(it->path().string());
        const auto output = convert_asset_bytes(ext.substr(1), input, stats);
        if (output.size() != input.size() || !std::equal(output.begin(), output.end(), input.begin())) write_replacement(it->path(), output);
    }
    return "Asset conversion complete\n"
           "Files scanned: " + std::to_string(stats.files_scanned) + "\n"
           "Gen9 to Legacy conversions: YTD=" + std::to_string(stats.converted_ytd) +
           ", YDR=" + std::to_string(stats.converted_ydr) +
           ", YDD=" + std::to_string(stats.converted_ydd) +
           ", YFT=" + std::to_string(stats.converted_yft) +
           ", YPT=" + std::to_string(stats.converted_ypt) + "\n"
           "Edition-independent resources parsed/rebuilt: YBN=" + std::to_string(stats.validated_ybn) +
           ", YMAP=" + std::to_string(stats.rebuilt_ymap) +
           ", YTYP=" + std::to_string(stats.rebuilt_ytyp) + "\n"
           "Already Legacy resources: " + std::to_string(stats.already_legacy) + "\n"
           "Other files preserved unchanged: " + std::to_string(stats.unchanged_other) + "\n"
           "Scanned bytes: " + std::to_string(stats.bytes_scanned) + "\n"
           "Audio, scripts, metadata outside YMAP/YTYP, and unsupported formats were preserved unchanged."
}
}
