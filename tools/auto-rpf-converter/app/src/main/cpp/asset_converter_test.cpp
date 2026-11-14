#include "asset_converter.h"
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>
#include "ragekit/ragekit.h"

static void require(bool condition, const char* message) {
    if (!condition) throw std::runtime_error(message);
}

static std::vector<std::uint8_t> bytes_of(const std::string& value) {
    return {value.begin(), value.end()};
}

int main() {
    require(rk::rsc::game_of("ydr", 154) == rk::Game::Enhanced, "YDR version 154 must be recognized as Gen9");
    require(rk::rsc::game_of("ydr", 159) == rk::Game::Enhanced, "YDR version 159 must be recognized as Gen9");
    require(rk::rsc::game_of("yft", 171) == rk::Game::Enhanced, "YFT version 171 must be recognized as Gen9");
    require(rk::rsc::game_of("ypt", 71) == rk::Game::Enhanced, "YPT version 71 must be recognized as Gen9");

    rk::Texture texture;
    texture.name = "converter_smoke";
    texture.width = 4;
    texture.height = 4;
    texture.levels = 1;
    texture.format = rk::TextureFormat::DXT1;
    texture.data.assign(8, 0x5A);
    rk::TextureDictionary dictionary;
    dictionary.add(texture);

    const auto gen9 = rk::write_ytd(dictionary, rk::Game::Enhanced);
    require(rk::rsc::Resource::load(gen9).version == 5, "writer must produce Gen9 YTD version 5");

    gta_asset_converter::ConversionStats stats;
    const auto legacy = gta_asset_converter::convert_asset_bytes("YTD", gen9, stats);
    require(rk::rsc::Resource::load(legacy).version == 13, "converter must produce Legacy YTD version 13");
    require(stats.converted_ytd == 1, "Gen9 YTD conversion count must be one");

    const auto parsed = rk::read_ytd(legacy);
    require(parsed.textures.size() == 1, "converted YTD must retain one texture");
    require(parsed.textures[0].name == "converter_smoke", "converted YTD must retain texture name");
    require(parsed.textures[0].data == texture.data, "converted YTD must retain texture pixels");

    const auto unchanged = gta_asset_converter::convert_asset_bytes("ytd", legacy, stats);
    require(unchanged == legacy, "Legacy YTD must be preserved byte-for-byte");
    require(stats.already_legacy == 1, "already-Legacy count must be one");

    // Exercise the complete Gen9 -> Legacy writer path for every supported resource family.
    // These synthetic, structurally valid resources prove the converter invokes the format readers
    // and writers, rather than changing file extensions or copying the original bytes.
    rk::Drawable empty_drawable;
    empty_drawable.name = "converter_test_drawable";

    gta_asset_converter::ConversionStats model_stats;
    const auto gen9_ydr = rk::write_ydr(empty_drawable, rk::Game::Enhanced);
    require(rk::rsc::Resource::load(gen9_ydr).version == 159, "synthetic YDR fixture must be Gen9");
    const auto legacy_ydr = gta_asset_converter::convert_asset_bytes("ydr", gen9_ydr, model_stats);
    require(rk::rsc::Resource::load(legacy_ydr).version == 165, "YDR converter must write Legacy version 165");
    (void)rk::read_ydr(legacy_ydr);
    require(model_stats.converted_ydr == 1, "Gen9 YDR conversion count must be one");

    rk::DrawableDictionary drawable_dictionary;
    drawable_dictionary.hashes.push_back(rk::joaat_lower("converter_test_drawable"));
    drawable_dictionary.drawables.push_back(empty_drawable);
    const auto gen9_ydd = rk::write_ydd(drawable_dictionary, rk::Game::Enhanced);
    require(rk::rsc::Resource::load(gen9_ydd).version == 159, "synthetic YDD fixture must be Gen9");
    const auto legacy_ydd = gta_asset_converter::convert_asset_bytes("ydd", gen9_ydd, model_stats);
    require(rk::rsc::Resource::load(legacy_ydd).version == 165, "YDD converter must write Legacy version 165");
    require(rk::read_ydd(legacy_ydd).drawables.size() == 1, "converted YDD must retain its drawable");
    require(model_stats.converted_ydd == 1, "Gen9 YDD conversion count must be one");

    rk::Fragment fragment;
    fragment.name = "converter_test_fragment";
    const auto gen9_yft = rk::write_yft(fragment, rk::Game::Enhanced);
    require(rk::rsc::Resource::load(gen9_yft).version == 171, "synthetic YFT fixture must be Gen9");
    const auto legacy_yft = gta_asset_converter::convert_asset_bytes("yft", gen9_yft, model_stats);
    require(rk::rsc::Resource::load(legacy_yft).version == 162, "YFT converter must write Legacy version 162");
    (void)rk::read_yft(legacy_yft);
    require(model_stats.converted_yft == 1, "Gen9 YFT conversion count must be one");

    rk::ParticleDictionary particles;
    particles.name = "converter_test_particles";
    const auto gen9_ypt = rk::write_ypt(particles, rk::Game::Enhanced);
    require(rk::rsc::Resource::load(gen9_ypt).version == 71, "synthetic YPT fixture must be Gen9");
    const auto legacy_ypt = gta_asset_converter::convert_asset_bytes("ypt", gen9_ypt, model_stats);
    require(rk::rsc::Resource::load(legacy_ypt).version == 68, "YPT converter must write Legacy version 68");
    (void)rk::read_ypt(legacy_ypt);
    require(model_stats.converted_ypt == 1, "Gen9 YPT conversion count must be one");

    // YBN/YMAP/YTYP share their RSC7 versions between Legacy and Enhanced. They are
    // parsed and rebuilt semantically instead of being incorrectly labelled Gen9 conversions.
    rk::BoundMaterial collision_material;
    const std::vector<rk::Vec3> collision_vertices = {{0,0,0}, {1,0,0}, {0,1,0}};
    const std::vector<uint32_t> collision_triangles = {0,1,2};
    auto collision_bound = rk::Bound::make_mesh(collision_vertices, collision_triangles, collision_material);
    const auto gen_ybn = rk::write_ybn(*collision_bound);
    gta_asset_converter::ConversionStats map_stats;
    const auto rebuilt_ybn = gta_asset_converter::convert_asset_bytes("ybn", gen_ybn, map_stats);
    require(rk::read_ybn(rebuilt_ybn) != nullptr, "rebuilt YBN must parse");
    require(map_stats.validated_ybn == 1, "YBN must be parsed and rebuilt");

    rk::MapData map_data;
    map_data.name = rk::joaat_lower("converter_test_map");
    const auto gen_ymap = rk::write_ymap(map_data);
    const auto rebuilt_ymap = gta_asset_converter::convert_asset_bytes("ymap", gen_ymap, map_stats);
    require(rk::read_ymap(rebuilt_ymap).name == map_data.name, "YMAP must retain its map name");
    require(map_stats.rebuilt_ymap == 1, "YMAP must be parsed and rebuilt");

    rk::MapTypes map_types;
    map_types.name = rk::joaat_lower("converter_test_types");
    const auto gen_ytyp = rk::write_ytyp(map_types);
    const auto rebuilt_ytyp = gta_asset_converter::convert_asset_bytes("ytyp", gen_ytyp, map_stats);
    require(rk::read_ytyp(rebuilt_ytyp).name == map_types.name, "YTYP must retain its dictionary name");
    require(map_stats.rebuilt_ytyp == 1, "YTYP must be parsed and rebuilt");

    // Legacy versions are recognized and preserved byte-for-byte for all five Gen9-capable extensions.
    const auto legacy_ydr_fixture = rk::write_ydr(empty_drawable, rk::Game::Legacy);
    const auto legacy_ydd_fixture = rk::write_ydd(drawable_dictionary, rk::Game::Legacy);
    const auto legacy_yft_fixture = rk::write_yft(fragment, rk::Game::Legacy);
    const auto legacy_ypt_fixture = rk::write_ypt(particles, rk::Game::Legacy);
    require(gta_asset_converter::convert_asset_bytes("ydr", legacy_ydr_fixture, model_stats) == legacy_ydr_fixture,
            "Legacy YDR must be preserved byte-for-byte");
    require(gta_asset_converter::convert_asset_bytes("ydd", legacy_ydd_fixture, model_stats) == legacy_ydd_fixture,
            "Legacy YDD must be preserved byte-for-byte");
    require(gta_asset_converter::convert_asset_bytes("yft", legacy_yft_fixture, model_stats) == legacy_yft_fixture,
            "Legacy YFT must be preserved byte-for-byte");
    require(gta_asset_converter::convert_asset_bytes("ypt", legacy_ypt_fixture, model_stats) == legacy_ypt_fixture,
            "Legacy YPT must be preserved byte-for-byte");
    require(model_stats.already_legacy == 4, "four already-Legacy model resources must be counted");

    bool malformed_supported_resource_rejected = false;
    try {
        (void)gta_asset_converter::convert_asset_bytes("ydr", std::vector<std::uint8_t>{1, 2, 3, 4}, model_stats);
    } catch (const std::exception&) {
        malformed_supported_resource_rejected = true;
    }
    require(malformed_supported_resource_rejected, "malformed supported resources must fail instead of being copied");
    
    const std::vector<std::uint8_t> opaque = {1, 2, 3, 4};
    const auto passthrough = gta_asset_converter::convert_asset_bytes("dat", opaque, stats);
    require(passthrough == opaque, "unsupported formats must be preserved unchanged");
    require(stats.unchanged_other == 1, "unchanged format count must be one");

    namespace fs = std::filesystem;
    const fs::path root = fs::temp_directory_path() / "gta-rpf-converter-directory-smoke";
    std::error_code ignored;
    fs::remove_all(root, ignored);
    fs::create_directories(root);
    rk::rsc::write_file((root / "smoke.ytd").string(), gen9);
    {
        std::ofstream note(root / "readme.txt", std::ios::binary);
        note << "leave me unchanged";
    }
    const std::string report = gta_asset_converter::convert_directory(root);
    require(report.find("YTD=1") != std::string::npos, "directory scan must report one converted YTD");
    require(report.find("Other files preserved unchanged: 1") != std::string::npos, "directory scan must preserve unrelated files");
    const auto directory_result = rk::rsc::read_file((root / "smoke.ytd").string());
    require(rk::rsc::Resource::load(directory_result).version == 13, "directory scan must replace Gen9 YTD with Legacy YTD");
    fs::remove_all(root, ignored);

    // A synthetic AES archive proves the app's key-folder import path is used by
    // the same ragekit reader that handles real PC RPF archives.
    const fs::path key_root = fs::temp_directory_path() / "gta-rpf-key-import-smoke";
    const fs::path archive_root = fs::temp_directory_path() / "gta-rpf-aes-archive-smoke";
    const fs::path output_root = fs::temp_directory_path() / "gta-rpf-aes-output-smoke";
    fs::remove_all(key_root, ignored);
    fs::remove_all(archive_root, ignored);
    fs::remove_all(output_root, ignored);
    fs::create_directories(key_root);
    fs::create_directories(archive_root);

    rk::RpfKeys test_keys;
    test_keys.aes.assign(32, 0x37);
    test_keys.save(key_root.string());
    require(gta_asset_converter::inspect_rpf_keys(key_root).find("AES-256: ready") != std::string::npos,
            "imported AES key must be detected");

    rk::RpfBuilder encrypted_builder;
    encrypted_builder.add_binary("common/data/keyed.txt", bytes_of("legitimate test key"), false);
    rk::RpfWriteOptions encrypted_options;
    encrypted_options.encryption = rk::RpfEncryption::Aes;
    encrypted_options.keys = std::make_shared<rk::RpfKeys>(test_keys);
    encrypted_options.name = "aes_fixture.rpf";
    const auto encrypted_bytes = encrypted_builder.build(encrypted_options);
    const fs::path encrypted_archive = archive_root / "aes_fixture.rpf";
    {
        std::ofstream out(encrypted_archive, std::ios::binary);
        out.write(reinterpret_cast<const char*>(encrypted_bytes.data()),
                  static_cast<std::streamsize>(encrypted_bytes.size()));
    }
    const auto keyed_report = gta_asset_converter::extract_rpf_archive(encrypted_archive, output_root, key_root);
    require(keyed_report.find("Encryption: AES") != std::string::npos,
            "encrypted archive must open with imported matching AES key");
    require(rk::rsc::read_file((output_root / "common/data/keyed.txt").string()) == bytes_of("legitimate test key"),
            "AES archive extraction must preserve the decrypted payload");

    const fs::path empty_keys = fs::temp_directory_path() / "gta-rpf-empty-keys-smoke";
    fs::remove_all(empty_keys, ignored);
    fs::create_directories(empty_keys);
    bool missing_key_rejected = false;
    try {
        (void)gta_asset_converter::extract_rpf_archive(encrypted_archive,
            fs::temp_directory_path() / "gta-rpf-no-key-output", empty_keys);
    } catch (const std::exception&) {
        missing_key_rejected = true;
    }
    require(missing_key_rejected, "encrypted RPF must fail closed without matching keys");

    fs::remove_all(key_root, ignored);
    fs::remove_all(archive_root, ignored);
    fs::remove_all(output_root, ignored);
    fs::remove_all(empty_keys, ignored);
    fs::remove_all(fs::temp_directory_path() / "gta-rpf-no-key-output", ignored);

    std::cout << "Gen9 conversion and keyed RPF extraction smoke tests passed\n";
    return 0;
}
