#include "asset_converter.h"
#include <algorithm>
#include <cctype>
#include <fstream>
#include <functional>
#include <limits>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <system_error>
#include <unordered_set>
#include <utility>
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
bool supported_resource_extension(const std::string& ext) {
    return ext == "ytd" || ext == "ydr" || ext == "ydd" || ext == "yft" || ext == "ypt" ||
           ext == "ybn" || ext == "ymap" || ext == "ytyp" || ext == "ycd" || ext == "ynd" ||
           ext == "yvr" || ext == "ywr" || ext == "yld" || ext == "yed" || ext == "yfd" ||
           ext == "ybd" || ext == "ynv";
}
int rel_type_for_extension(const std::string& ext) {
    if (ext == "dat4") return rk::reltype::Dat4;
    if (ext == "dat10") return rk::reltype::Dat10;
    if (ext == "dat15") return rk::reltype::Dat15;
    if (ext == "dat16") return rk::reltype::Dat16;
    if (ext == "dat22") return rk::reltype::Dat22;
    if (ext == "dat54") return rk::reltype::Dat54;
    if (ext == "dat149") return rk::reltype::Dat149;
    if (ext == "dat150") return rk::reltype::Dat150;
    if (ext == "dat151") return rk::reltype::Dat151;
    return 0;
}
bool is_binary_asset_extension(const std::string& ext) {
    return ext == "awc" || ext == "rel" || ext == "gxt2" || ext == "nametable" ||
           rel_type_for_extension(ext) != 0 || ext == "ymt" || ext == "ymf" ||
           ext == "ypdb" || ext == "pso" || ext == "cut";
}
bool is_metadata_container_extension(const std::string& ext) {
    return ext == "ymt" || ext == "ymf" || ext == "ypdb" || ext == "pso" || ext == "cut";
}
bool supported_extension(const std::string& ext) {
    return supported_resource_extension(ext) || is_binary_asset_extension(ext);
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
std::vector<std::uint8_t> convert_asset_bytes(const std::string& extension,
                                              std::span<const std::uint8_t> input,
                                              ConversionStats& stats,
                                              std::shared_ptr<const rk::RpfKeys> keys) {
    const std::string ext = lower(extension);
    if (!supported_extension(ext)) {
        ++stats.unchanged_other;
        return {input.begin(), input.end()};
    }
    if (input.size() > kMaxAssetBytes) throw std::runtime_error(ext + " exceeds the 512 MiB per-asset safety limit");

    // AWC is a binary audio container, not RSC7. Parse it, decode the PCM/IMA-ADPCM
    // streams that ragekit supports (including legitimate XXTEA decryption), and demand
    // an exact stored-byte round trip before preserving the original archive entry.
    // Other codecs, including Enhanced codec 7, remain opaque and are never transcoded.
    if (ext == "awc") {
        if (!rk::is_awc(input)) {
            ++stats.unchanged_other; // e.g. a shader cache that happens to use the .awc suffix
            return {input.begin(), input.end()};
        }
        const auto active_keys = keys ? keys : std::make_shared<rk::RpfKeys>();
        const rk::AwcFile awc = rk::read_awc(input, active_keys);
        const auto sounds = rk::awc_decode(awc, active_keys);
        const auto roundtrip = rk::write_awc(awc, active_keys);
        if (roundtrip.size() != input.size() || !std::equal(roundtrip.begin(), roundtrip.end(), input.begin())) {
            throw std::runtime_error("AWC parser/writer round trip changed stored bytes; refusing to repack it");
        }
        ++stats.validated_awc;
        for (const auto& sound : sounds) {
            if (sound.decoded()) ++stats.decoded_awc_streams;
            else ++stats.opaque_awc_streams;
        }
        return {input.begin(), input.end()};
    }

    // REL and DAT4/DAT10/DAT15/DAT16/DAT22/DAT54/DAT149/DAT150/DAT151
    // are byte-preserving audio-data containers in the pinned ragekit version.
    if (ext == "rel" || rel_type_for_extension(ext) != 0) {
        const rk::Rel rel = rk::read_rel(input);
        const int expected_type = rel_type_for_extension(ext);
        if (expected_type != 0 && rel.type != static_cast<std::uint32_t>(expected_type)) {
            throw std::runtime_error(ext + " contains REL type " + std::to_string(rel.type) +
                                     ", expected " + std::to_string(expected_type));
        }
        const auto roundtrip = rk::write_rel(rel);
        if (roundtrip.size() != input.size() || !std::equal(roundtrip.begin(), roundtrip.end(), input.begin())) {
            throw std::runtime_error("REL parser/writer round trip changed stored bytes");
        }
        ++stats.validated_rel;
        return {input.begin(), input.end()};
    }
    if (ext == "gxt2") {
        const rk::TextTable table = rk::read_gxt2(input);
        const auto roundtrip = rk::write_gxt2(table);
        if (roundtrip.size() != input.size() || !std::equal(roundtrip.begin(), roundtrip.end(), input.begin())) {
            throw std::runtime_error("GXT2 parser/writer round trip changed stored bytes");
        }
        ++stats.validated_gxt2;
        return {input.begin(), input.end()};
    }
    if (ext == "nametable") {
        const auto names = rk::read_nametable(input);
        const auto roundtrip = rk::write_nametable(names);
        if (roundtrip.size() != input.size() || !std::equal(roundtrip.begin(), roundtrip.end(), input.begin())) {
            throw std::runtime_error("REL nametable round trip changed stored bytes");
        }
        ++stats.validated_nametable;
        return {input.begin(), input.end()};
    }
    if (is_metadata_container_extension(ext)) {
        if (rk::is_pso(input)) {
            const rk::Pso pso = rk::read_pso(input);
            const auto roundtrip = rk::write_pso(pso);
            if (roundtrip.size() != input.size() || !std::equal(roundtrip.begin(), roundtrip.end(), input.begin())) {
                throw std::runtime_error("PSO parser/writer round trip changed stored bytes");
            }
            ++stats.validated_pso;
            return {input.begin(), input.end()};
        }
        if (rk::is_rbf(input)) {
            const rk::RbfNode rbf = rk::read_rbf(input);
            const auto roundtrip = rk::write_rbf(rbf);
            if (roundtrip.size() != input.size() || !std::equal(roundtrip.begin(), roundtrip.end(), input.begin())) {
                throw std::runtime_error("RBF parser/writer round trip changed stored bytes");
            }
            ++stats.validated_rbf;
            return {input.begin(), input.end()};
        }
        // .ymt/.ymf can be other data containers or RSC7 resources. Without a
        // recognized PSIN/RBF0 signature, preserve them but do not call them validated.
        ++stats.unchanged_other;
        return {input.begin(), input.end()};
    }

    const auto resource = rk::rsc::Resource::load(input);
    // Collision and map resources do not have a Gen9-specific layout: GTA V Legacy and
    // Enhanced both use YBN=43 and YMAP/YTYP=2. Parse and serialize them through ragekit
    // so malformed data is rejected and the semantic structures are rebuilt, not renamed.
    if (ext == "ybn") {
        if (resource.version != 43) throw std::runtime_error("YBN has unsupported RSC7 resource version " + std::to_string(resource.version));
        (void)rk::read_ybn(input);
        ++stats.validated_ybn;
        // YBN is edition-independent. Validate it, then preserve the original bytes to
        // avoid quantization or BVH reordering from an unnecessary serialize cycle.
        return {input.begin(), input.end()};
    }
    if (ext == "ymap") {
        if (resource.version != 2) throw std::runtime_error("YMAP has unsupported RSC7 resource version " + std::to_string(resource.version));
        (void)rk::read_ymap(input);
        ++stats.validated_ymap;
        return {input.begin(), input.end()};
    }
    if (ext == "ytyp") {
        if (resource.version != 2) throw std::runtime_error("YTYP has unsupported RSC7 resource version " + std::to_string(resource.version));
        (void)rk::read_ytyp(input);
        ++stats.validated_ytyp;
        return {input.begin(), input.end()};
    }

    // These resource families have the same on-disk layouts in GTA V Legacy and
    // Enhanced. Parse them with the pinned ragekit readers, then preserve the
    // original RSC7 bytes rather than introducing an unnecessary rewrite.
    if (ext == "ycd") {
        size_t census = 0;
        std::string where;
        (void)rk::read_ycd_checked(input, &census, &where);
        if (census != 0) throw std::runtime_error("YCD has unrecognized pointer-like fields: " + where);
        ++stats.validated_ycd;
        return {input.begin(), input.end()};
    }
    if (ext == "ynd") {
        (void)rk::read_ynd(input);
        ++stats.validated_ynd;
        return {input.begin(), input.end()};
    }
    if (ext == "yvr") {
        (void)rk::read_yvr(input);
        ++stats.validated_yvr;
        return {input.begin(), input.end()};
    }
    if (ext == "ywr") {
        (void)rk::read_ywr(input);
        ++stats.validated_ywr;
        return {input.begin(), input.end()};
    }
    if (ext == "yld") {
        (void)rk::read_yld(input);
        ++stats.validated_yld;
        return {input.begin(), input.end()};
    }
    if (ext == "yed") {
        size_t census = 0;
        std::string where;
        (void)rk::read_yed(input, &census, &where);
        if (census != 0) throw std::runtime_error("YED has unrecognized pointer-like fields: " + where);
        ++stats.validated_yed;
        return {input.begin(), input.end()};
    }
    if (ext == "yfd") {
        size_t census = 0;
        std::string where;
        (void)rk::read_yfd(input, &census, &where);
        if (census != 0) throw std::runtime_error("YFD has unrecognized pointer-like fields: " + where);
        ++stats.validated_yfd;
        return {input.begin(), input.end()};
    }
    if (ext == "ybd") {
        (void)rk::read_ybd(input);
        ++stats.validated_ybd;
        return {input.begin(), input.end()};
    }
    if (ext == "ynv") {
        (void)rk::read_ynv(input);
        ++stats.validated_ynv;
        return {input.begin(), input.end()};
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
        for (const auto& block : particles.blocks()) {
            if (block->type == rk::PtxType::Behaviour &&
                block->get<std::uint32_t>(8) == 0x843768CFu) {
                throw std::runtime_error("YPT contains the Enhanced-only ptfx_g9 behaviour; Legacy conversion is unsafe");
            }
        }
        auto output = rk::write_ypt(particles, rk::Game::Legacy);
        ++stats.converted_ypt;
        return output;
    }
    throw std::runtime_error("no conversion handler for " + ext);
}
static fs::path safe_archive_relative_path(const std::string& raw) {
    if (raw.empty() || raw.find(':') != std::string::npos) {
        throw std::runtime_error("empty or drive-qualified RPF path rejected");
    }
    std::string normalized = raw;
    std::replace(normalized.begin(), normalized.end(), '\\', '/');
    const fs::path path(normalized);
    if (path.is_absolute()) throw std::runtime_error("absolute RPF path rejected: " + raw);
    fs::path clean;
    for (const auto& component : path) {
        if (component == "." || component.empty()) continue;
        if (component == ".." || component == path.root_name() || component == path.root_directory()) {
            throw std::runtime_error("unsafe RPF path rejected: " + raw);
        }
        clean /= component;
    }
    if (clean.empty()) throw std::runtime_error("empty RPF path rejected");
    return clean;
}
static std::string rpf_encryption_name(rk::RpfEncryption encryption) {
    switch (encryption) {
        case rk::RpfEncryption::None: return "None";
        case rk::RpfEncryption::Open: return "OPEN";
        case rk::RpfEncryption::Aes: return "AES";
        case rk::RpfEncryption::Ng: return "NG";
    }
    return "Unknown";
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
           "Edition-independent resources validated and preserved: YBN=" + std::to_string(stats.validated_ybn) +
           ", YMAP=" + std::to_string(stats.validated_ymap) +
           ", YTYP=" + std::to_string(stats.validated_ytyp) +
           ", YCD=" + std::to_string(stats.validated_ycd) +
           ", YND=" + std::to_string(stats.validated_ynd) +
           ", YVR=" + std::to_string(stats.validated_yvr) +
           ", YWR=" + std::to_string(stats.validated_ywr) +
           ", YLD=" + std::to_string(stats.validated_yld) +
           ", YED=" + std::to_string(stats.validated_yed) +
           ", YFD=" + std::to_string(stats.validated_yfd) +
           ", YBD=" + std::to_string(stats.validated_ybd) +
           ", YNV=" + std::to_string(stats.validated_ynv) + "\n"
           "Audio/metadata validated and preserved: AWC=" + std::to_string(stats.validated_awc) +
           " (decoded streams=" + std::to_string(stats.decoded_awc_streams) +
           ", opaque codecs=" + std::to_string(stats.opaque_awc_streams) +
           "), REL/DAT=" + std::to_string(stats.validated_rel) +
           ", GXT2=" + std::to_string(stats.validated_gxt2) +
           ", PSO=" + std::to_string(stats.validated_pso) +
           ", RBF=" + std::to_string(stats.validated_rbf) +
           ", nametable=" + std::to_string(stats.validated_nametable) + "\n" +
           "Already Legacy resources: " + std::to_string(stats.already_legacy) + "\n"
           "Other files preserved unchanged: " + std::to_string(stats.unchanged_other) + "\n"
           "Scanned bytes: " + std::to_string(stats.bytes_scanned) + "\n"
           "Audio, scripts, metadata outside YMAP/YTYP, and unsupported formats were preserved unchanged.";
}

std::string inspect_rpf_keys(const fs::path& key_directory) {
    const rk::RpfKeys keys = rk::RpfKeys::load(key_directory.string());
    std::ostringstream report;
    report << "RPF key status\n"
           << "AES-256: " << (keys.has_aes() ? "ready" : "missing") << "\n"
           << "NG key material: " << (keys.has_ng() ? "ready" : "incomplete") << "\n"
           << "AWC audio key: " << (keys.has_awc() ? "ready" : "missing") << "\n"
           << "Key files remain in the app-private storage folder on this device.";
    return report.str();
}

std::string extract_rpf_archive(const fs::path& source,
                                const fs::path& destination,
                                const fs::path& key_directory) {
    constexpr std::uint64_t max_archive_bytes = 1024ull * 1024ull * 1024ull;
    constexpr std::uint64_t max_entry_bytes = 512ull * 1024ull * 1024ull;
    constexpr std::uint64_t max_total_bytes = 8ull * 1024ull * 1024ull * 1024ull;
    constexpr std::size_t max_entries = 1'000'000;

    const auto source_status = fs::symlink_status(source);
    if (fs::is_symlink(source_status) || !fs::is_regular_file(source_status)) {
        throw std::runtime_error("RPF source must be a regular, non-symlink file");
    }
    if (fs::file_size(source) > max_archive_bytes) {
        throw std::runtime_error("RPF archive exceeds the 1 GiB input limit");
    }

    auto keys = std::make_shared<rk::RpfKeys>(rk::RpfKeys::load(key_directory.string()));
    const rk::Rpf archive = rk::Rpf::open(source.string(), keys);
    const auto& entries = archive.entries();
    if (entries.size() > max_entries) throw std::runtime_error("RPF entry count exceeds one million");

    fs::create_directories(destination);
    const fs::path root = fs::canonical(destination);
    std::unordered_set<std::string> seen_paths;
    std::uint64_t total_bytes = 0;
    std::size_t files_written = 0;

    for (std::size_t i = 0; i < entries.size(); ++i) {
        const rk::RpfEntry& entry = entries[i];
        if (entry.is_directory()) continue;
        if (entry.size() > max_entry_bytes) {
            throw std::runtime_error("RPF entry exceeds the 512 MiB limit: " + entry.path);
        }
        const fs::path relative = safe_archive_relative_path(entry.path);
        std::string path_key = relative.generic_string();
        std::transform(path_key.begin(), path_key.end(), path_key.begin(),
                       [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
        if (!seen_paths.insert(path_key).second) {
            throw std::runtime_error("duplicate case-insensitive RPF path rejected: " + entry.path);
        }

        const fs::path target = root / relative;
        fs::path current = root;
        for (const auto& component : relative.parent_path()) {
            current /= component;
            std::error_code status_error;
            const auto status = fs::symlink_status(current, status_error);
            if (!status_error && fs::exists(status)) {
                if (fs::is_symlink(status) || !fs::is_directory(status)) {
                    throw std::runtime_error("unsafe RPF output parent rejected: " + current.string());
                }
            } else if (!fs::create_directory(current)) {
                throw std::runtime_error("cannot create RPF output directory: " + current.string());
            }
        }

        std::error_code target_error;
        const auto target_status = fs::symlink_status(target, target_error);
        if (!target_error && fs::exists(target_status)) {
            throw std::runtime_error("RPF output file already exists: " + target.string());
        }

        const auto payload = archive.extract(static_cast<int>(i));
        if (payload.size() > max_entry_bytes) {
            throw std::runtime_error("extracted RPF entry exceeds the 512 MiB limit: " + entry.path);
        }
        if (payload.size() > std::numeric_limits<std::uint64_t>::max() - total_bytes) {
            throw std::runtime_error("RPF extracted byte count overflow");
        }
        total_bytes += payload.size();
        if (total_bytes > max_total_bytes) {
            throw std::runtime_error("RPF extracted output exceeds the 8 GiB limit");
        }

        std::ofstream out(target, std::ios::binary | std::ios::trunc);
        if (!out) throw std::runtime_error("cannot create extracted RPF file: " + target.string());
        out.write(reinterpret_cast<const char*>(payload.data()), static_cast<std::streamsize>(payload.size()));
        out.flush();
        if (!out) throw std::runtime_error("cannot write extracted RPF file: " + target.string());
        ++files_written;
    }

    return "Extraction complete\n"
           "Encryption: " + rpf_encryption_name(archive.encryption()) + "\n"
           "Entries: " + std::to_string(entries.size()) + "\n"
           "Files written: " + std::to_string(files_written) + "\n"
           "Bytes written: " + std::to_string(total_bytes);
}
std::string build_open_rpf7(const fs::path& source_directory, const fs::path& output_path) {
    constexpr std::uint64_t max_file_bytes = 512ull * 1024ull * 1024ull;
    constexpr std::uint64_t max_total_input_bytes = 512ull * 1024ull * 1024ull;
    constexpr std::uint64_t max_output_bytes = 1024ull * 1024ull * 1024ull;
    constexpr std::size_t max_files = 1'000'000;

    const auto root_status = fs::symlink_status(source_directory);
    if (fs::is_symlink(root_status) || !fs::is_directory(root_status)) {
        throw std::runtime_error("RPF7 build input must be a regular directory, not a symlink");
    }
    const fs::path root = fs::canonical(source_directory);
    if (fs::exists(output_path)) throw std::runtime_error("RPF7 output already exists; refusing to overwrite it");

    rk::RpfBuilder builder;
    std::uint64_t total_input_bytes = 0;
    std::size_t file_count = 0;
    for (fs::recursive_directory_iterator it(root), end; it != end; ++it) {
        const auto status = it->symlink_status();
        if (fs::is_symlink(status)) {
            throw std::runtime_error("symbolic links are not allowed in RPF7 build input: " + it->path().string());
        }
        if (!fs::is_regular_file(status)) continue;
        if (++file_count > max_files) throw std::runtime_error("RPF7 build exceeds one million files");

        const std::uint64_t size = fs::file_size(it->path());
        if (size > max_file_bytes) {
            throw std::runtime_error("RPF7 build file exceeds the 512 MiB per-file limit: " + it->path().string());
        }
        if (size > max_total_input_bytes - total_input_bytes) {
            throw std::runtime_error("RPF7 build input exceeds the 512 MiB aggregate memory limit");
        }
        total_input_bytes += size;

        const fs::path relative = it->path().lexically_relative(root);
        const std::string archive_path = relative.generic_string();
        (void)safe_archive_relative_path(archive_path);
        auto data = rk::rsc::read_file(it->path().string());
        if (data.size() != size) throw std::runtime_error("short read while packaging: " + it->path().string());
        builder.add_file(archive_path, std::move(data));
    }
    if (file_count == 0) throw std::runtime_error("RPF7 build input contains no files");

    rk::RpfWriteOptions options;
    options.encryption = rk::RpfEncryption::Open;
    options.name = output_path.filename().string();
    options.compress = true;
    if (!output_path.parent_path().empty()) {
        fs::create_directories(output_path.parent_path());
    }
    fs::path temporary = output_path;
    temporary += ".tmp";
    std::error_code ignored;
    fs::remove(temporary, ignored);

    // RpfBuilder::build() materializes the entire archive in an ostringstream and then
    // copies it into a Bytes vector. The upstream save() API writes the same CodeWalker-
    // compatible RPF7 layout directly to a file, avoiding that additional archive-sized
    // memory allocation on Android. Source file buffers are still held by the builder,
    // so the 512 MiB aggregate input limit remains in force.
    try {
        builder.save(temporary.string(), options);
        const std::uint64_t archive_bytes = fs::file_size(temporary);
        if (archive_bytes > max_output_bytes) {
            throw std::runtime_error("generated RPF7 exceeds the 1 GiB output limit");
        }

        auto no_keys = std::make_shared<rk::RpfKeys>();
        const rk::Rpf parsed = rk::Rpf::open(temporary.string(), no_keys);
        if (parsed.encryption() != rk::RpfEncryption::Open) {
            throw std::runtime_error("RPF7 output is not OPEN/unencrypted");
        }
        const std::size_t parsed_files = static_cast<std::size_t>(std::count_if(
            parsed.entries().begin(), parsed.entries().end(),
            [](const rk::RpfEntry& entry) { return !entry.is_directory(); }));
        if (parsed_files != file_count) {
            throw std::runtime_error("RPF7 validation entry count does not match packaged file count");
        }
        fs::rename(temporary, output_path);
        return "RPF7 build complete\n"
               "Encryption: OPEN\n"
               "Files: " + std::to_string(file_count) + "\n"
               "Input bytes: " + std::to_string(total_input_bytes) + "\n"
               "Archive bytes: " + std::to_string(archive_bytes) + "\n"
               "Validated with the pinned ragekit RPF7 reader.";
    } catch (...) {
        fs::remove(temporary, ignored);
        throw;
    }
}

}


namespace gta_asset_converter {
namespace {
constexpr std::uint64_t kDirectMaxArchiveBytes = 0xFFFFFFFFull;
constexpr std::uint64_t kDirectMaxEntryBytes = 512ull * 1024ull * 1024ull;
constexpr std::uint64_t kDirectMaxNestedArchiveBytes = 256ull * 1024ull * 1024ull;
constexpr std::uint64_t kDirectMaxConvertedResourceBytes = 512ull * 1024ull * 1024ull;
constexpr std::uint64_t kDirectMaxOutputBytes = 0xFFFFFFFFull;
constexpr std::size_t kDirectMaxEntries = 1'000'000;
constexpr std::size_t kDirectMaxNestedArchives = 500;
constexpr int kDirectMaxNestedDepth = 8;
constexpr int kDirectMaxDirectoryDepth = 64;

struct ArchivePreflight {
    std::size_t total_entries = 0;
    std::size_t nested_archives = 0;
};

void preflight_archive_tree(const rk::Rpf& archive, int nested_depth, ArchivePreflight& stats);

void preflight_archive_directory(const rk::Rpf& archive,
                                 int directory_index,
                                 int directory_depth,
                                 int nested_depth,
                                 ArchivePreflight& stats,
                                 std::size_t& visited,
                                 std::unordered_set<std::string>& paths) {
    if (directory_depth > kDirectMaxDirectoryDepth) {
        throw std::runtime_error("RPF directory nesting exceeds the 64-level safety limit");
    }

    for (int child_index : archive.children(directory_index)) {
        ++visited;
        ++stats.total_entries;
        if (stats.total_entries > kDirectMaxEntries) {
            throw std::runtime_error("RPF tree exceeds the one-million-entry safety limit");
        }

        const rk::RpfEntry& entry = archive.entries().at(static_cast<std::size_t>(child_index));
        const std::string relative = safe_archive_relative_path(entry.path).generic_string();
        if (!paths.insert(lower(relative)).second) {
            throw std::runtime_error("duplicate case-insensitive RPF path rejected: " + entry.path);
        }

        if (entry.is_directory()) {
            preflight_archive_directory(archive, child_index, directory_depth + 1,
                                        nested_depth, stats, visited, paths);
            continue;
        }

        if (entry.size() > kDirectMaxEntryBytes) {
            throw std::runtime_error("RPF entry exceeds the 512 MiB per-entry safety limit: " + entry.path);
        }

        if (entry.is_archive()) {
            ++stats.nested_archives;
            if (stats.nested_archives > kDirectMaxNestedArchives) {
                throw std::runtime_error("RPF tree exceeds the 500 nested-archive safety limit");
            }
            if (nested_depth >= kDirectMaxNestedDepth) {
                throw std::runtime_error("nested RPF depth exceeds the supported limit of 8; no output was written");
            }
            if (entry.size() > kDirectMaxNestedArchiveBytes) {
                throw std::runtime_error("nested RPF exceeds the 256 MiB in-memory rebuild limit: " + entry.path);
            }

            // Opening the child validates its RPF7 header and requires legitimate matching
            // keys for AES/NG archives. We never copy an unreadable encrypted child silently.
            rk::Rpf child = archive.open_child(child_index);
            if (child.byte_size() > kDirectMaxNestedArchiveBytes) {
                throw std::runtime_error("nested RPF exceeds the 256 MiB in-memory rebuild limit: " + entry.path);
            }
            preflight_archive_tree(child, nested_depth + 1, stats);
        }
    }
}

void preflight_archive_tree(const rk::Rpf& archive, int nested_depth, ArchivePreflight& stats) {
    const auto& entries = archive.entries();
    if (entries.empty() || entries.size() > kDirectMaxEntries) {
        throw std::runtime_error("RPF entry table is empty or exceeds one million entries");
    }

    ++stats.total_entries; // count this archive's root directory
    if (stats.total_entries > kDirectMaxEntries) {
        throw std::runtime_error("RPF tree exceeds the one-million-entry safety limit");
    }

    std::size_t visited = 1;
    std::unordered_set<std::string> paths;
    preflight_archive_directory(archive, 0, 0, nested_depth, stats, visited, paths);
    if (visited != entries.size()) {
        throw std::runtime_error("RPF contains unreachable table entries; refusing to drop them during rebuild");
    }
}

struct DirectArchiveConversionStats {
    ConversionStats assets;
    std::uint64_t inspected_resource_bytes = 0;
    std::size_t nested_rebuilt = 0;
    std::vector<std::pair<std::string, std::uint32_t>> converted_resources;
};

void convert_archive_resources(const rk::Rpf& archive,
                               rk::RpfBuilder& builder,
                               int directory_index,
                               const std::string& archive_prefix,
                               DirectArchiveConversionStats& stats,
                               const std::shared_ptr<const rk::RpfKeys>& keys) {
    for (int child_index : archive.children(directory_index)) {
        const rk::RpfEntry& entry = archive.entries().at(static_cast<std::size_t>(child_index));
        const std::string relative = safe_archive_relative_path(entry.path).generic_string();

        if (entry.is_directory()) {
            convert_archive_resources(archive, builder, child_index, archive_prefix, stats, keys);
            continue;
        }

        ++stats.assets.files_scanned;

        if (entry.is_archive()) {
            rk::Rpf child = archive.open_child(child_index);
            rk::RpfBuilder* child_builder = builder.nested(relative);
            if (child_builder == nullptr) {
                throw std::runtime_error("RAGEKIT did not expose the nested archive builder for " + entry.path);
            }
            convert_archive_resources(child, *child_builder, 0, archive_prefix + relative + "/", stats, keys);
            ++stats.nested_rebuilt;
            continue;
        }

        std::string extension = lower(fs::path(entry.name).extension().string());
        if (!extension.empty() && extension.front() == '.') extension.erase(extension.begin());
        if (!supported_extension(extension)) {
            ++stats.assets.unchanged_other;
            continue; // the builder keeps the original entry lazy, including encrypted .ysc data
        }

        if (is_binary_asset_extension(extension)) {
            if (!entry.is_binary()) {
                if (extension == "ymt" || extension == "ymf") {
                    ++stats.assets.unchanged_other; // an RSC7/unknown .ymt or .ymf is not guessed as PSO/RBF
                    continue;
                }
                throw std::runtime_error("binary audio/metadata extension is stored as an RSC7 resource: " + entry.path);
            }
            if (entry.size() > kDirectMaxEntryBytes) {
                throw std::runtime_error("binary audio/metadata entry exceeds the 512 MiB safety limit: " + entry.path);
            }
            if (entry.size() > kDirectMaxConvertedResourceBytes - stats.inspected_resource_bytes) {
                throw std::runtime_error("supported asset inspection exceeds the 512 MiB aggregate memory budget; no output was written");
            }
            const auto original = archive.extract(child_index);
            if (original.size() > kDirectMaxEntryBytes) {
                throw std::runtime_error("extracted audio/metadata entry exceeds the 512 MiB safety limit: " + entry.path);
            }
            stats.inspected_resource_bytes += original.size();
            stats.assets.bytes_scanned += original.size();
            const auto checked = convert_asset_bytes(extension, original, stats.assets, keys);
            if (checked != original) {
                throw std::runtime_error("binary validator unexpectedly changed " + entry.path +
                                         "; only byte-preserving validation is enabled for this format");
            }
            continue; // keep the original source-backed entry and its compression
        }

        if (!entry.is_resource()) {
            throw std::runtime_error("supported RSC7 resource extension is stored as a binary entry: " + entry.path);
        }
        if (entry.size() > kDirectMaxEntryBytes) {
            throw std::runtime_error("supported resource exceeds the 512 MiB conversion limit: " + entry.path);
        }
        if (entry.size() > kDirectMaxConvertedResourceBytes - stats.inspected_resource_bytes) {
            throw std::runtime_error("supported resource conversion exceeds the 512 MiB aggregate memory budget; no output was written");
        }

        auto original = archive.extract(child_index);
        if (original.size() > kDirectMaxEntryBytes) {
            throw std::runtime_error("extracted resource exceeds the 512 MiB conversion limit: " + entry.path);
        }
        stats.inspected_resource_bytes += entry.size();
        stats.assets.bytes_scanned += original.size();

        auto converted = convert_asset_bytes(extension, original, stats.assets, keys);
        const bool changed = converted.size() != original.size() ||
            !std::equal(converted.begin(), converted.end(), original.begin());
        if (changed) {
            builder.add_file(relative, std::move(converted));
            const int expected_version = legacy_version(extension);
            if (expected_version <= 0) {
                throw std::runtime_error("converted resource has no known Legacy version: " + entry.path);
            }
            stats.converted_resources.emplace_back(archive_prefix + relative,
                                                   static_cast<std::uint32_t>(expected_version));
        }
        // If unchanged, leave the RAGEKIT source-backed node alone. This preserves the
        // original compression and, for .ysc, the encrypted stored bytes and resource flags.
    }
}

void verify_open_archive_tree(const rk::Rpf& source, const rk::Rpf& output, int depth) {
    if (depth > kDirectMaxNestedDepth) {
        throw std::runtime_error("output nested RPF depth exceeds the validated limit");
    }
    if (output.encryption() != rk::RpfEncryption::Open &&
        output.encryption() != rk::RpfEncryption::None) {
        throw std::runtime_error("rebuilt nested archive is not OPEN/unencrypted: " + output.name());
    }
    if (source.entries().size() != output.entries().size()) {
        throw std::runtime_error("RPF output entry count differs from its source: " + output.name());
    }

    for (std::size_t i = 1; i < source.entries().size(); ++i) {
        const rk::RpfEntry& source_entry = source.entries()[i];
        const int output_index = output.find(source_entry.path);
        if (output_index < 0) {
            throw std::runtime_error("RPF output is missing source path: " + source_entry.path);
        }
        const rk::RpfEntry& output_entry = output.entries().at(static_cast<std::size_t>(output_index));
        if (source_entry.is_directory() != output_entry.is_directory() ||
            source_entry.is_resource() != output_entry.is_resource() ||
            source_entry.is_binary() != output_entry.is_binary()) {
            throw std::runtime_error("RPF output entry kind changed: " + source_entry.path);
        }
        // RAGEKIT path lookup is case-insensitive. Also require the exact stored spelling
        // to survive the rebuild so case-sensitive output consumers do not see renamed paths.
        if (source_entry.name != output_entry.name || source_entry.path != output_entry.path) {
            throw std::runtime_error("RPF output changed entry name or path spelling: " + source_entry.path);
        }
        // Non-archive binary payloads are pass-through entries. Their logical size must
        // remain identical after the OPEN rewrite; nested RPF sizes can change because
        // their child archives are rebuilt, and resource sizes can change during conversion.
        if (source_entry.is_binary() && !source_entry.is_archive() &&
            source_entry.size() != output_entry.size()) {
            throw std::runtime_error("RPF output changed unchanged binary size: " + source_entry.path);
        }
        if (source_entry.is_archive()) {
            rk::Rpf source_child = source.open_child(static_cast<int>(i));
            rk::Rpf output_child = output.open_child(output_index);
            verify_open_archive_tree(source_child, output_child, depth + 1);
        }
    }
}
} // namespace

std::string convert_rpf_archive_to_open_rpf7(const fs::path& source_path,
                                             const fs::path& output_path,
                                             const fs::path& key_directory) {
    const auto source_status = fs::symlink_status(source_path);
    if (fs::is_symlink(source_status) || !fs::is_regular_file(source_status)) {
        throw std::runtime_error("RPF source must be a regular, non-symlink file");
    }
    const std::uint64_t input_bytes = fs::file_size(source_path);
    if (input_bytes > kDirectMaxArchiveBytes) {
        throw std::runtime_error("RPF7 input exceeds the 4 GiB address/key-size safety limit");
    }
    if (fs::exists(output_path)) {
        throw std::runtime_error("RPF7 output already exists; refusing to overwrite it");
    }

    auto keys = std::make_shared<rk::RpfKeys>(rk::RpfKeys::load(key_directory.string()));
    const rk::Rpf source = rk::Rpf::open(source_path.string(), keys);
    ArchivePreflight preflight;
    preflight_archive_tree(source, 0, preflight);

    // from_archive keeps source entries file-backed/lazy. Only supported resources are
    // extracted into memory; unchanged binaries, compressed files and encrypted .ysc
    // resources retain their original stored representation.
    rk::RpfBuilder builder = rk::RpfBuilder::from_archive(source, rk::RpfBuilder::Nested::Rewrite);
    DirectArchiveConversionStats stats;
    convert_archive_resources(source, builder, 0, "", stats, keys);

    rk::RpfWriteOptions options;
    options.encryption = rk::RpfEncryption::Open;
    options.name = output_path.filename().string();
    options.compress = true;

    if (!output_path.parent_path().empty()) {
        fs::create_directories(output_path.parent_path());
    }
    fs::path temporary = output_path;
    temporary += ".tmp";
    std::error_code ignored;
    fs::remove(temporary, ignored);

    try {
        builder.save(temporary.string(), options);
        const std::uint64_t output_bytes = fs::file_size(temporary);
        if (output_bytes > kDirectMaxOutputBytes) {
            throw std::runtime_error("generated RPF7 exceeds the 4 GiB output safety limit");
        }

        const rk::Rpf output = rk::Rpf::open(temporary.string(), std::make_shared<rk::RpfKeys>());
        verify_open_archive_tree(source, output, 0);
        if (output.encryption() != rk::RpfEncryption::Open) {
            throw std::runtime_error("top-level RPF7 output is not OPEN/unencrypted");
        }
        for (const auto& converted : stats.converted_resources) {
            const auto location = output.resolve(converted.first);
            if (!location.archive || location.entry < 0) {
                throw std::runtime_error("converted resource is missing from output: " + converted.first);
            }
            const rk::RpfEntry& entry = location.archive->entries().at(static_cast<std::size_t>(location.entry));
            if (!entry.is_resource() || entry.resource_version() != converted.second) {
                throw std::runtime_error("converted resource version was not preserved in output: " + converted.first);
            }
        }

        fs::rename(temporary, output_path);
        return "RPF7 conversion complete\n"
               "Input encryption: " + rpf_encryption_name(source.encryption()) + "\n"
               "Output encryption: OPEN\n"
               "Files examined: " + std::to_string(stats.assets.files_scanned) + "\n"
               "Gen9 to Legacy conversions: YTD=" + std::to_string(stats.assets.converted_ytd) +
               ", YDR=" + std::to_string(stats.assets.converted_ydr) +
               ", YDD=" + std::to_string(stats.assets.converted_ydd) +
               ", YFT=" + std::to_string(stats.assets.converted_yft) +
               ", YPT=" + std::to_string(stats.assets.converted_ypt) + "\n"
               "Edition-independent resources validated and preserved: YBN=" + std::to_string(stats.assets.validated_ybn) +
               ", YMAP=" + std::to_string(stats.assets.validated_ymap) +
               ", YTYP=" + std::to_string(stats.assets.validated_ytyp) + "\n"
               "Audio/metadata validated and preserved: AWC=" + std::to_string(stats.assets.validated_awc) +
               " (decoded streams=" + std::to_string(stats.assets.decoded_awc_streams) +
               ", opaque codecs=" + std::to_string(stats.assets.opaque_awc_streams) +
               "), REL/DAT=" + std::to_string(stats.assets.validated_rel) +
               ", GXT2=" + std::to_string(stats.assets.validated_gxt2) +
               ", PSO=" + std::to_string(stats.assets.validated_pso) +
               ", RBF=" + std::to_string(stats.assets.validated_rbf) +
               ", nametable=" + std::to_string(stats.assets.validated_nametable) + "\n" +
               "Already Legacy resources preserved: " + std::to_string(stats.assets.already_legacy) + "\n"
               "Other files preserved without format conversion: " + std::to_string(stats.assets.unchanged_other) + "\n"
               "Nested RPFs rebuilt: " + std::to_string(stats.nested_rebuilt) +
               " of " + std::to_string(preflight.nested_archives) + "\n"
               "Supported asset bytes inspected: " + std::to_string(stats.assets.bytes_scanned) + "\n"
               "Archive bytes: " + std::to_string(output_bytes) + "\n"
               "Verified: RPF7 tree, nested OPEN archives, paths, entry kinds and converted resource versions.\n"
               "Unchanged entries are copied from the source lazily; encrypted .ysc stored bytes are preserved.";
    } catch (...) {
        fs::remove(temporary, ignored);
        throw;
    }
}



static std::string inspect_dlc_pack_metadata_archive(const rk::Rpf& archive,
                                                       const std::string& archive_label) {
    constexpr std::uint64_t max_xml_bytes = 8ull * 1024ull * 1024ull;
    constexpr std::size_t max_data_files = 100'000;

    auto read_xml_entry = [&](const rk::Rpf::Location& location, const std::string& label) {
        if (!location.archive || location.entry < 0) {
            throw std::runtime_error("DLC archive is missing " + label);
        }
        const rk::RpfEntry& entry =
            location.archive->entries().at(static_cast<std::size_t>(location.entry));
        if (entry.is_directory()) {
            throw std::runtime_error("DLC metadata path is a directory: " + label);
        }
        if (entry.size() > max_xml_bytes) {
            throw std::runtime_error("DLC XML exceeds the 8 MiB safety limit: " + label);
        }
        const auto bytes = location.archive->extract(location.entry);
        if (bytes.empty() || bytes.size() > max_xml_bytes) {
            throw std::runtime_error("DLC XML is empty or oversized: " + label);
        }
        return std::string(reinterpret_cast<const char*>(bytes.data()), bytes.size());
    };

    const auto setup_location = archive.resolve("setup2.xml");
    const std::string setup_text = read_xml_entry(setup_location, "setup2.xml");
    const rk::XmlElement setup = rk::parse_xml(setup_text);
    if (lower(setup.name) != "ssetupdata") {
        throw std::runtime_error("setup2.xml root must be SSetupData");
    }

    const std::string device_name = setup.child_text("deviceName");
    const std::string dat_file = setup.child_text("datFile");
    const std::string pack_type = setup.child_text("type");
    if (lower(device_name).rfind("dlc_", 0) != 0 ||
        device_name.find_first_of("/:\\\\") != std::string::npos ||
        device_name.find("..") != std::string::npos) {
        throw std::runtime_error("setup2.xml has an invalid deviceName");
    }
    if (dat_file.empty()) {
        throw std::runtime_error("setup2.xml does not declare datFile");
    }
    const std::string safe_dat_file = safe_archive_relative_path(dat_file).generic_string();
    const auto content_location = archive.resolve(safe_dat_file);
    const std::string content_text = read_xml_entry(content_location, dat_file);
    const rk::XmlElement content = rk::parse_xml(content_text);
    if (lower(content.name) != "cdatafilemgr__contentsofdatafilexml") {
        throw std::runtime_error("DLC datFile root is not CDataFileMgr__ContentsOfDataFileXml");
    }

    const rk::XmlElement* data_files = content.child_ci("dataFiles");
    if (data_files == nullptr) {
        throw std::runtime_error("DLC content XML is missing dataFiles");
    }

    std::string order_value = "not specified";
    if (const rk::XmlElement* order = setup.child_ci("order")) {
        const std::string value = order->attribute("value");
        if (!value.empty()) order_value = value;
    }

    std::size_t declared = 0;
    std::size_t resolved = 0;
    std::size_t missing = 0;
    std::size_t unsafe = 0;
    std::size_t prefix_mismatch = 0;
    std::size_t platform_dependent = 0;
    std::size_t wrong_rpf_kind = 0;
    std::size_t rpf_files = 0;
    std::size_t empty_entries = 0;
    std::vector<std::string> warnings;
    const std::string expected_prefix = device_name + ":/";
    const std::string expected_prefix_lower = lower(expected_prefix);

    for (const rk::XmlElement& item : data_files->children) {
        if (lower(item.name) != "item") continue;
        if (++declared > max_data_files) {
            throw std::runtime_error("DLC content XML exceeds the 100,000 data-file safety limit");
        }

        const std::string filename = item.child_text("filename");
        const std::string file_type = item.child_text("fileType");
        if (filename.empty() || file_type.empty()) {
            ++empty_entries;
            if (warnings.size() < 20) warnings.push_back("dataFiles Item is missing filename or fileType");
            continue;
        }

        const std::string filename_lower = lower(filename);
        if (filename_lower.rfind(expected_prefix_lower, 0) != 0) {
            ++prefix_mismatch;
            if (warnings.size() < 20) {
                warnings.push_back("device path does not match setup2.xml deviceName: " + filename);
            }
            continue;
        }

        const std::string relative_raw = filename.substr(expected_prefix.size());
        if (lower(relative_raw).find("%platform%") != std::string::npos) {
            ++platform_dependent;
            continue;
        }

        fs::path relative;
        try {
            relative = safe_archive_relative_path(relative_raw);
        } catch (const std::exception&) {
            ++unsafe;
            if (warnings.size() < 20) warnings.push_back("unsafe content path rejected: " + filename);
            continue;
        }

        const auto location = archive.resolve(relative.generic_string());
        if (!location.archive || location.entry < 0) {
            ++missing;
            if (warnings.size() < 20) warnings.push_back("referenced file is absent from this DLC RPF: " + filename);
            continue;
        }

        const rk::RpfEntry& target =
            location.archive->entries().at(static_cast<std::size_t>(location.entry));
        if (target.is_directory()) {
            ++missing;
            if (warnings.size() < 20) warnings.push_back("content path resolves to a directory: " + filename);
            continue;
        }

        if (lower(file_type) == "rpf_file") {
            ++rpf_files;
            if (!target.is_archive()) {
                ++wrong_rpf_kind;
                if (warnings.size() < 20) warnings.push_back("RPF_FILE does not resolve to a nested RPF archive: " + filename);
                continue;
            }
        }
        ++resolved;
    }

    std::size_t change_sets = 0;
    if (const rk::XmlElement* sets = content.child_ci("contentChangeSets")) {
        for (const auto& child : sets->children) {
            if (lower(child.name) == "item") ++change_sets;
        }
    }
    std::size_t change_set_groups = 0;
    if (const rk::XmlElement* groups = setup.child_ci("contentChangeSetGroups")) {
        for (const auto& child : groups->children) {
            if (lower(child.name) == "item") ++change_set_groups;
        }
    }

    std::ostringstream report;
    report << "DLC metadata validation complete\n"
           << "Archive: " << archive_label << "\n"
           << "Input encryption: " << rpf_encryption_name(archive.encryption()) << "\n"
           << "Device name: " << device_name << "\n"
           << "Pack type: " << (pack_type.empty() ? "not specified" : pack_type) << "\n"
           << "Load order: " << order_value << "\n"
           << "Metadata file: " << safe_dat_file << "\n"
           << "Data files declared: " << declared << "\n"
           << "Data files resolved: " << resolved << "\n"
           << "RPF_FILE entries: " << rpf_files << "\n"
           << "Missing references: " << missing << "\n"
           << "Unsafe references: " << unsafe << "\n"
           << "Device-name mismatches: " << prefix_mismatch << "\n"
           << "RPF_FILE type mismatches: " << wrong_rpf_kind << "\n"
           << "Platform-dependent paths not resolved: " << platform_dependent << "\n"
           << "Incomplete data-file entries: " << empty_entries << "\n"
           << "Content change sets: " << change_sets << "\n"
           << "Setup change-set groups: " << change_set_groups << "\n"
           << "Source DLC archive was not modified.\n";
    if (!warnings.empty()) {
        report << "Warnings:\n";
        for (const auto& warning : warnings) report << "- " << warning << "\n";
        if (missing + unsafe + prefix_mismatch + wrong_rpf_kind + empty_entries > warnings.size()) {
            report << "- Additional warnings omitted.\n";
        }
    }
    return report.str();
}

std::string inspect_dlc_pack_metadata(const fs::path& source_path,
                                      const fs::path& key_directory) {
    constexpr std::uint64_t max_archive_bytes = 0xFFFFFFFFull;
    const auto source_status = fs::symlink_status(source_path);
    if (fs::is_symlink(source_status) || !fs::is_regular_file(source_status)) {
        throw std::runtime_error("DLC source must be a regular, non-symlink RPF file");
    }
    if (fs::file_size(source_path) > max_archive_bytes) {
        throw std::runtime_error("DLC RPF exceeds the 4 GiB RPF7 safety limit");
    }
    auto keys = std::make_shared<rk::RpfKeys>(rk::RpfKeys::load(key_directory.string()));
    const rk::Rpf archive = rk::Rpf::open(source_path.string(), keys);
    return inspect_dlc_pack_metadata_archive(archive, source_path.filename().string());
}

std::string inspect_platform_dlc_pack_metadata(const fs::path& base_archive_path,
                                               const std::string& pack_name,
                                               const fs::path& key_directory) {
    constexpr std::uint64_t max_archive_bytes = 0xFFFFFFFFull;
    const auto source_status = fs::symlink_status(base_archive_path);
    if (fs::is_symlink(source_status) || !fs::is_regular_file(source_status)) {
        throw std::runtime_error("base x64 archive must be a regular, non-symlink file");
    }
    if (fs::file_size(base_archive_path) > max_archive_bytes) {
        throw std::runtime_error("base x64 archive exceeds the 4 GiB RPF7 safety limit");
    }
    if (pack_name.empty() || pack_name == "." || pack_name == ".." ||
        pack_name.find_first_of("/\\:") != std::string::npos ||
        pack_name.find("..") != std::string::npos) {
        throw std::runtime_error("unsafe platform DLC pack name");
    }
    for (unsigned char c : pack_name) {
        if (c < 0x20 || c == 0x7F) throw std::runtime_error("control character in platform DLC pack name");
    }

    auto keys = std::make_shared<rk::RpfKeys>(rk::RpfKeys::load(key_directory.string()));
    const rk::Rpf base = rk::Rpf::open(base_archive_path.string(), keys);
    const std::string internal_path = "dlcpacks/" + pack_name + "/dlc.rpf";
    const auto location = base.resolve(internal_path);
    if (!location.archive || location.entry < 0) {
        return "PLATFORM_DLC_NOT_FOUND\n";
    }

    const rk::RpfEntry& entry =
        location.archive->entries().at(static_cast<std::size_t>(location.entry));
    if (entry.is_directory() || !entry.is_archive() || entry.compressed() || entry.encrypted) {
        return "PLATFORM_DLC_INVALID_ENTRY\nBase archive: " + base.name() +
               "\nInternal path: " + internal_path +
               "\nExpected a plain, uncompressed nested dlc.rpf archive entry.\n";
    }
    if (entry.size() == 0 || entry.size() > max_archive_bytes) {
        return "PLATFORM_DLC_INVALID_ENTRY\nBase archive: " + base.name() +
               "\nInternal path: " + internal_path + "\nNested dlc.rpf is empty or exceeds the 4 GiB limit.\n";
    }

    try {
        // open_child is file-backed: the nested DLC is not materialized as a multi-hundred-MiB buffer.
        // RAGEKIT uses the nested entry's real basename (dlc.rpf) for NG key selection.
        const rk::Rpf child = location.archive->open_child(location.entry);
        const std::string label = base.name() + ":" + internal_path;
        return "PLATFORM_DLC_FOUND\nBase archive: " + base.name() +
               "\nInternal path: " + internal_path + "\n" +
               inspect_dlc_pack_metadata_archive(child, label);
    } catch (const std::exception& error) {
        const std::string message = error.what();
        const bool missing_keys =
            message.find("no AES keys are available") != std::string::npos ||
            message.find("no NG keys are available") != std::string::npos;
        return std::string(missing_keys ? "PLATFORM_DLC_FOUND_LOCKED\n" : "PLATFORM_DLC_FOUND_INVALID\n") +
               "Base archive: " + base.name() + "\nInternal path: " + internal_path +
               "\nMetadata inspection failed: " + message + "\n";
    }
}


std::string read_dlc_list_entries(const fs::path& update_rpf_path,
                                  const fs::path& key_directory) {
    constexpr std::uint64_t max_archive_bytes = 0xFFFFFFFFull;
    constexpr std::uint64_t max_xml_bytes = 8ull * 1024ull * 1024ull;
    constexpr std::size_t max_items = 1024;

    const auto source_status = fs::symlink_status(update_rpf_path);
    if (fs::is_symlink(source_status) || !fs::is_regular_file(source_status)) {
        throw std::runtime_error("update.rpf must be a regular, non-symlink file");
    }
    if (fs::file_size(update_rpf_path) > max_archive_bytes) {
        throw std::runtime_error("update.rpf exceeds the 4 GiB RPF7 safety limit");
    }

    auto keys = std::make_shared<rk::RpfKeys>(rk::RpfKeys::load(key_directory.string()));
    const rk::Rpf update = rk::Rpf::open(update_rpf_path.string(), keys);
    const auto location = update.resolve("common/data/dlclist.xml");
    if (!location.archive || location.entry < 0) {
        throw std::runtime_error("update.rpf is missing common/data/dlclist.xml");
    }
    const rk::RpfEntry& entry =
        location.archive->entries().at(static_cast<std::size_t>(location.entry));
    if (entry.is_directory() || entry.size() > max_xml_bytes) {
        throw std::runtime_error("dlclist.xml is a directory or exceeds the 8 MiB limit");
    }
    const auto bytes = location.archive->extract(location.entry);
    if (bytes.empty() || bytes.size() > max_xml_bytes) {
        throw std::runtime_error("dlclist.xml is empty or exceeds the 8 MiB limit");
    }

    const std::string xml(reinterpret_cast<const char*>(bytes.data()), bytes.size());
    const rk::XmlElement root = rk::parse_xml(xml);
    const rk::XmlElement* paths =
        lower(root.name) == "paths" ? &root : root.child_ci("Paths");
    if (paths == nullptr) {
        throw std::runtime_error("dlclist.xml is missing SMandatoryPacksData/Paths");
    }

    std::ostringstream report;
    report << "DLC_LIST_V1\n";
    std::size_t item_index = 0;
    for (const rk::XmlElement& item : paths->children) {
        if (lower(item.name) != "item") continue;
        if (++item_index > max_items) {
            throw std::runtime_error("dlclist.xml exceeds the 1,024 Item safety limit");
        }

        std::string value = item.text;
        std::replace(value.begin(), value.end(), '\\', '/');
        const std::string value_lower = lower(value);

        auto emit_pack = [&](const std::string& prefix, const std::string& kind) {
            if (value_lower.rfind(prefix, 0) != 0) return false;
            std::string pack = value.substr(prefix.size());
            if (!pack.empty() && pack.back() == '/') pack.pop_back();
            bool safe = !pack.empty() && pack != "." && pack != ".." &&
                        pack.find_first_of("/\\:") == std::string::npos;
            for (unsigned char c : pack) {
                if (c < 0x20 || c == 0x7F) safe = false;
            }
            if (!safe) {
                report << "UNSAFE|" << item_index << "\n";
            } else {
                report << kind << "|" << pack << "\n";
            }
            return true;
        };

        if (value.empty()) {
            report << "UNSUPPORTED|" << item_index << "\n";
        } else if (!emit_pack("dlcpacks:/", "DLC") &&
                   !emit_pack("platform:/dlcpacks/", "PLATFORM")) {
            report << "UNSUPPORTED|" << item_index << "\n";
        }
    }
    return report.str();
}


} // namespace gta_asset_converter
