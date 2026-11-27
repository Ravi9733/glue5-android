#include "asset_converter.h"
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>
#include <utility>
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

    // A known Enhanced-only particle behaviour cannot be made safe for Legacy by
    // changing only the YPT resource version. Build a small graph fixture and ensure
    // the converter rejects it instead of producing a misleading Legacy file.
    rk::ParticleDictionary enhanced_only_particles;
    auto rule_dictionary = std::make_shared<rk::PtxBlock>();
    rule_dictionary->type = rk::PtxType::RuleDictionary;
    rule_dictionary->bytes.resize(0x40);
    rule_dictionary->set<std::uint16_t>(0x38, 1);
    rule_dictionary->set<std::uint16_t>(0x3A, 1);
    auto particle_rule_array = std::make_shared<rk::PtxBlock>();
    particle_rule_array->type = rk::PtxType::PointerArray;
    particle_rule_array->bytes.resize(8);
    auto particle_rule = std::make_shared<rk::PtxBlock>();
    particle_rule->type = rk::PtxType::ParticleRule;
    particle_rule->bytes.resize(0x240);
    particle_rule->set<std::uint16_t>(0x130, 1);
    particle_rule->set<std::uint16_t>(0x132, 1);
    auto behaviour_array = std::make_shared<rk::PtxBlock>();
    behaviour_array->type = rk::PtxType::PointerArray;
    behaviour_array->bytes.resize(8);
    auto enhanced_only_behaviour = std::make_shared<rk::PtxBlock>();
    enhanced_only_behaviour->type = rk::PtxType::Behaviour;
    enhanced_only_behaviour->bytes.resize(0xD0);
    enhanced_only_behaviour->set<std::uint32_t>(8, 0x843768CFu);
    rule_dictionary->links.push_back({0x30, particle_rule_array, 0});
    particle_rule_array->links.push_back({0, particle_rule, 0});
    particle_rule->links.push_back({0x128, behaviour_array, 0});
    behaviour_array->links.push_back({0, enhanced_only_behaviour, 0});
    enhanced_only_particles.particle_rules.dictionary = rule_dictionary;
    const auto gen9_ypt_with_gen9_behaviour = rk::write_ypt(enhanced_only_particles, rk::Game::Enhanced);
    bool gen9_behaviour_rejected = false;
    try {
        (void)gta_asset_converter::convert_asset_bytes("ypt", gen9_ypt_with_gen9_behaviour, model_stats);
    } catch (const std::exception&) {
        gen9_behaviour_rejected = true;
    }
    require(gen9_behaviour_rejected, "YPT with Enhanced-only ptfx_g9 behaviour must fail closed");

    // YBN/YMAP/YTYP share their RSC7 versions between Legacy and Enhanced. They are
    // parsed for validation and preserved byte-for-byte; they are not Gen9 conversions.
    rk::BoundMaterial collision_material;
    const std::vector<rk::Vec3> collision_vertices = {{0,0,0}, {1,0,0}, {0,1,0}};
    const std::vector<uint32_t> collision_triangles = {0,1,2};
    auto collision_bound = rk::Bound::make_mesh(collision_vertices, collision_triangles, collision_material);
    const auto gen_ybn = rk::write_ybn(*collision_bound);
    gta_asset_converter::ConversionStats map_stats;
    const auto validated_ybn = gta_asset_converter::convert_asset_bytes("ybn", gen_ybn, map_stats);
    require(rk::read_ybn(validated_ybn) != nullptr, "YBN must parse");
    require(validated_ybn == gen_ybn, "edition-independent YBN must be preserved byte-for-byte");
    require(map_stats.validated_ybn == 1, "YBN must be parsed and validated");

    rk::MapData map_data;
    map_data.name = rk::joaat_lower("converter_test_map");
    const auto gen_ymap = rk::write_ymap(map_data);
    const auto validated_ymap = gta_asset_converter::convert_asset_bytes("ymap", gen_ymap, map_stats);
    require(rk::read_ymap(validated_ymap).name == map_data.name, "YMAP must retain its map name");
    require(validated_ymap == gen_ymap, "edition-independent YMAP must be preserved byte-for-byte");
    require(map_stats.validated_ymap == 1, "YMAP must be parsed and validated");

    rk::MapTypes map_types;
    map_types.name = rk::joaat_lower("converter_test_types");
    const auto gen_ytyp = rk::write_ytyp(map_types);
    const auto validated_ytyp = gta_asset_converter::convert_asset_bytes("ytyp", gen_ytyp, map_stats);
    require(rk::read_ytyp(validated_ytyp).name == map_types.name, "YTYP must retain its dictionary name");
    require(validated_ytyp == gen_ytyp, "edition-independent YTYP must be preserved byte-for-byte");
    require(map_stats.validated_ytyp == 1, "YTYP must be parsed and validated");

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

    // End-to-end resource conversion + OPEN RPF7 repacking. This exercises every
    // currently supported resource family through the same folder pipeline used by Android.
    const fs::path pipeline_root = fs::temp_directory_path() / "gta-rpf-full-pipeline-input";
    const fs::path pipeline_output = fs::temp_directory_path() / "gta-rpf-full-pipeline-output.rpf";
    fs::remove_all(pipeline_root, ignored);
    fs::remove(pipeline_output, ignored);
    fs::create_directories(pipeline_root / "dlcpacks" / "test");
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/texture.ytd").string(), gen9);
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/model.ydr").string(), gen9_ydr);
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/models.ydd").string(), gen9_ydd);
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/vehicle.yft").string(), gen9_yft);
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/particles.ypt").string(), gen9_ypt);
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/collision.ybn").string(), gen_ybn);
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/map.ymap").string(), gen_ymap);
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/types.ytyp").string(), gen_ytyp);
    const auto untouched_data = bytes_of("unsupported metadata stays byte-for-byte");
    rk::rsc::write_file((pipeline_root / "dlcpacks/test/extra.dat").string(), untouched_data);

    const std::string pipeline_report = gta_asset_converter::convert_directory(pipeline_root);
    require(pipeline_report.find("YTD=1") != std::string::npos &&
            pipeline_report.find("YDR=1") != std::string::npos &&
            pipeline_report.find("YDD=1") != std::string::npos &&
            pipeline_report.find("YFT=1") != std::string::npos &&
            pipeline_report.find("YPT=1") != std::string::npos,
            "folder pipeline must report all five Gen9-to-Legacy conversions");
    require(pipeline_report.find("YBN=1") != std::string::npos &&
            pipeline_report.find("YMAP=1") != std::string::npos &&
            pipeline_report.find("YTYP=1") != std::string::npos,
            "folder pipeline must report edition-independent resource validation");

    const std::string package_report = gta_asset_converter::build_open_rpf7(pipeline_root, pipeline_output);
    require(package_report.find("RPF7 build complete") == 0, "full conversion pipeline must create an RPF7 archive");
    const rk::Rpf pipeline_archive = rk::Rpf::open(pipeline_output.string(), std::make_shared<rk::RpfKeys>());
    require(pipeline_archive.encryption() == rk::RpfEncryption::Open, "full pipeline output must be OPEN");
    auto read_pipeline_file = [&](const std::string& path) {
        const int index = pipeline_archive.find(path);
        require(index >= 0, "full pipeline output must preserve every relative asset path");
        return pipeline_archive.extract(index);
    };
    require(rk::rsc::Resource::load(read_pipeline_file("dlcpacks/test/texture.ytd")).version == 13,
            "RPF output must contain Legacy YTD version 13");
    require(rk::rsc::Resource::load(read_pipeline_file("dlcpacks/test/model.ydr")).version == 165,
            "RPF output must contain Legacy YDR version 165");
    require(rk::rsc::Resource::load(read_pipeline_file("dlcpacks/test/models.ydd")).version == 165,
            "RPF output must contain Legacy YDD version 165");
    require(rk::rsc::Resource::load(read_pipeline_file("dlcpacks/test/vehicle.yft")).version == 162,
            "RPF output must contain Legacy YFT version 162");
    require(rk::rsc::Resource::load(read_pipeline_file("dlcpacks/test/particles.ypt")).version == 68,
            "RPF output must contain Legacy YPT version 68");
    require(rk::read_ybn(read_pipeline_file("dlcpacks/test/collision.ybn")) != nullptr,
            "RPF output YBN must parse");
    require(rk::read_ymap(read_pipeline_file("dlcpacks/test/map.ymap")).name == map_data.name,
            "RPF output YMAP must retain its map name");
    require(rk::read_ytyp(read_pipeline_file("dlcpacks/test/types.ytyp")).name == map_types.name,
            "RPF output YTYP must retain its archetype dictionary name");
    require(read_pipeline_file("dlcpacks/test/extra.dat") == untouched_data,
            "unsupported files must remain byte-for-byte unchanged in the RPF output");
    fs::remove_all(pipeline_root, ignored);
    fs::remove(pipeline_output, ignored);

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
    const fs::path aes_repacked_path = archive_root / "aes_fixture_android.rpf";
    fs::remove(aes_repacked_path, ignored);
    const auto aes_repack_report = gta_asset_converter::convert_rpf_archive_to_open_rpf7(
        encrypted_archive, aes_repacked_path, key_root);
    require(aes_repack_report.find("RPF7 conversion complete") == 0,
            "direct converter must read a matching-key AES source archive");
    const rk::Rpf aes_repacked = rk::Rpf::open(
        aes_repacked_path.string(), std::make_shared<rk::RpfKeys>());
    require(aes_repacked.encryption() == rk::RpfEncryption::Open,
            "AES source must be rebuilt as OPEN RPF7");
    const int aes_payload_index = aes_repacked.find("common/data/keyed.txt");
    require(aes_payload_index >= 0 &&
            aes_repacked.extract(aes_payload_index) == bytes_of("legitimate test key"),
            "AES source payload must survive direct OPEN repacking");


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
    const fs::path aes_missing_direct_output = archive_root / "aes_missing_direct_keys.rpf";
    fs::remove(aes_missing_direct_output, ignored);
    bool direct_aes_missing_key_rejected = false;
    try {
        (void)gta_asset_converter::convert_rpf_archive_to_open_rpf7(
            encrypted_archive, aes_missing_direct_output, empty_keys);
    } catch (const std::exception&) {
        direct_aes_missing_key_rejected = true;
    }
    require(direct_aes_missing_key_rejected && !fs::exists(aes_missing_direct_output),
            "direct AES conversion must reject missing keys without creating output");

    // A deterministic invertible synthetic NG table exercises the NG key-file import
    // and archive-name/size-dependent decryption path without using or bundling game keys.
    const fs::path ng_key_root = fs::temp_directory_path() / "gta-rpf-ng-key-import-smoke";
    const fs::path ng_archive_root = fs::temp_directory_path() / "gta-rpf-ng-archive-smoke";
    const fs::path ng_output_root = fs::temp_directory_path() / "gta-rpf-ng-output-smoke";
    fs::remove_all(ng_key_root, ignored);
    fs::remove_all(ng_archive_root, ignored);
    fs::remove_all(ng_output_root, ignored);
    fs::create_directories(ng_key_root);
    fs::create_directories(ng_archive_root);

    rk::RpfKeys ng_test_keys;
    ng_test_keys.ng_keys.assign(101 * 272, 0);
    ng_test_keys.hash_lut.resize(256);
    for (std::size_t i = 0; i < ng_test_keys.hash_lut.size(); ++i) {
        ng_test_keys.hash_lut[i] = static_cast<std::uint8_t>(i);
    }
    static constexpr int round_b[4][4] = {
        {0, 7, 10, 13}, {1, 4, 11, 14}, {2, 5, 8, 15}, {3, 6, 9, 12}
    };
    ng_test_keys.ng_tables.resize(17 * 16 * 256);
    for (int round = 0; round < 17; ++round) {
        for (int word = 0; word < 4; ++word) {
            for (int lane = 0; lane < 4; ++lane) {
                const int table = (round <= 1 || round == 16) ? 4 * word + lane : round_b[word][lane];
                for (int value = 0; value < 256; ++value) {
                    ng_test_keys.ng_tables[(round * 16 + table) * 256 + value] =
                        static_cast<std::uint32_t>(value) << (lane * 8);
                }
            }
        }
    }
    require(ng_test_keys.has_ng(), "synthetic NG key material must satisfy the exact imported file sizes");
    ng_test_keys.save(ng_key_root.string());
    require(gta_asset_converter::inspect_rpf_keys(ng_key_root).find("NG key material: ready") != std::string::npos,
            "imported NG keys, tables and hash LUT must be detected");

    rk::RpfBuilder ng_builder;
    ng_builder.add_binary("common/data/ng-keyed.txt", bytes_of("synthetic NG payload"), false);
    rk::RpfWriteOptions ng_options;
    ng_options.encryption = rk::RpfEncryption::Ng;
    ng_options.keys = std::make_shared<rk::RpfKeys>(ng_test_keys);
    ng_options.name = "ng_fixture.rpf";
    const auto ng_bytes = ng_builder.build(ng_options);
    const fs::path ng_archive = ng_archive_root / "ng_fixture.rpf";
    {
        std::ofstream out(ng_archive, std::ios::binary);
        out.write(reinterpret_cast<const char*>(ng_bytes.data()),
                  static_cast<std::streamsize>(ng_bytes.size()));
    }
    const auto ng_report = gta_asset_converter::extract_rpf_archive(ng_archive, ng_output_root, ng_key_root);
    require(ng_report.find("Encryption: NG") != std::string::npos,
            "NG archive must open with matching imported synthetic key material");
    require(rk::rsc::read_file((ng_output_root / "common/data/ng-keyed.txt").string()) == bytes_of("synthetic NG payload"),
            "NG archive extraction must preserve the decrypted payload");
    const fs::path ng_repacked_path = ng_archive_root / "ng_fixture_android.rpf";
    fs::remove(ng_repacked_path, ignored);
    const auto ng_repack_report = gta_asset_converter::convert_rpf_archive_to_open_rpf7(
        ng_archive, ng_repacked_path, ng_key_root);
    require(ng_repack_report.find("RPF7 conversion complete") == 0,
            "direct converter must read a matching-key NG source archive");
    const rk::Rpf ng_repacked = rk::Rpf::open(
        ng_repacked_path.string(), std::make_shared<rk::RpfKeys>());
    require(ng_repacked.encryption() == rk::RpfEncryption::Open,
            "NG source must be rebuilt as OPEN RPF7");
    const int ng_payload_index = ng_repacked.find("common/data/ng-keyed.txt");
    require(ng_payload_index >= 0 &&
            ng_repacked.extract(ng_payload_index) == bytes_of("synthetic NG payload"),
            "NG source payload must survive direct OPEN repacking");


    bool missing_ng_rejected = false;
    try {
        (void)gta_asset_converter::extract_rpf_archive(
            ng_archive, fs::temp_directory_path() / "gta-rpf-ng-no-key-output", empty_keys);
    } catch (const std::exception&) {
        missing_ng_rejected = true;
    }
    require(missing_ng_rejected, "NG archive must fail closed when matching NG key material is absent");
    const fs::path ng_missing_direct_output = ng_archive_root / "ng_missing_direct_keys.rpf";
    fs::remove(ng_missing_direct_output, ignored);
    bool direct_ng_missing_key_rejected = false;
    try {
        (void)gta_asset_converter::convert_rpf_archive_to_open_rpf7(
            ng_archive, ng_missing_direct_output, empty_keys);
    } catch (const std::exception&) {
        direct_ng_missing_key_rejected = true;
    }
    require(direct_ng_missing_key_rejected && !fs::exists(ng_missing_direct_output),
            "direct NG conversion must reject missing keys without creating output");

    fs::remove_all(ng_key_root, ignored);
    fs::remove_all(ng_archive_root, ignored);
    fs::remove_all(ng_output_root, ignored);
    fs::remove_all(fs::temp_directory_path() / "gta-rpf-ng-no-key-output", ignored);

    fs::remove_all(key_root, ignored);
    fs::remove_all(archive_root, ignored);
    fs::remove_all(output_root, ignored);
    fs::remove_all(empty_keys, ignored);
    fs::remove_all(fs::temp_directory_path() / "gta-rpf-no-key-output", ignored);

    // A nested RPF larger than 16 MiB must survive the OPEN RPF7 writer.
    const fs::path large_source = fs::temp_directory_path() / "gta-rpf-large-nested-source";
    const fs::path large_output = fs::temp_directory_path() / "gta-rpf-large-nested-output.rpf";
    fs::remove_all(large_source, ignored);
    fs::remove(large_output, ignored);
    fs::create_directories(large_source / "dlcpacks" / "test");
    rk::rsc::write_file((large_source / "textures.ytd").string(), legacy);

    std::vector<std::uint8_t> large_payload(17u * 1024u * 1024u);
    std::uint32_t random_state = 0x12345678u;
    for (auto& byte : large_payload) {
        random_state ^= random_state << 13;
        random_state ^= random_state >> 17;
        random_state ^= random_state << 5;
        byte = static_cast<std::uint8_t>(random_state >> 24);
    }
    rk::RpfBuilder child_builder;
    child_builder.add_binary("big.bin", large_payload, false);
    rk::RpfWriteOptions child_options;
    child_options.encryption = rk::RpfEncryption::Open;
    child_options.name = "nested.rpf";
    const auto child_bytes = child_builder.build(child_options);
    require(child_bytes.size() > 0xFFFFFFu, "nested test archive must exceed 16 MiB");
    {
        std::ofstream out(large_source / "dlcpacks" / "test" / "nested.rpf", std::ios::binary);
        out.write(reinterpret_cast<const char*>(child_bytes.data()),
                  static_cast<std::streamsize>(child_bytes.size()));
    }
    const auto large_report = gta_asset_converter::build_open_rpf7(large_source, large_output);
    require(large_report.find("RPF7 build complete") == 0, "RAGEKIT writer must build the outer RPF7");
    const rk::Rpf outer = rk::Rpf::open(large_output.string(), std::make_shared<rk::RpfKeys>());
    const int nested_index = outer.find("dlcpacks/test/nested.rpf");
    require(nested_index >= 0, "outer archive must contain the nested RPF");
    require(outer.entries()[static_cast<std::size_t>(nested_index)].stored_size == 0 &&
            outer.entries()[static_cast<std::size_t>(nested_index)].uncompressed_size == child_bytes.size(),
            "large nested RPF must use the uncompressed 32-bit binary size field");
    const auto nested_bytes = outer.extract(nested_index);
    const rk::Rpf nested_archive = rk::Rpf::open_memory(nested_bytes, "nested.rpf", std::make_shared<rk::RpfKeys>());
    const int big_index = nested_archive.find("big.bin");
    require(big_index >= 0 && nested_archive.extract(big_index) == large_payload,
            "nested RPF larger than 16 MiB must round trip without truncation");
    const int texture_index = outer.find("textures.ytd");
    require(texture_index >= 0 && outer.extract(texture_index) == legacy,
            "RPF7 builder must preserve RSC7 resource flags and payload");

    fs::remove_all(large_source, ignored);
    fs::remove(large_output, ignored);

    // Direct archive-to-archive conversion: nested Gen9 assets are converted without
    // extracting the whole archive tree, while unsupported data and encrypted .ysc
    // stored bytes remain source-backed and unchanged.
    const fs::path direct_key_root = fs::temp_directory_path() / "gta-rpf-direct-key-root";
    const fs::path direct_source_path = fs::temp_directory_path() / "gta-rpf-direct-source.rpf";
    const fs::path direct_output_path = fs::temp_directory_path() / "gta-rpf-direct-output.rpf";
    fs::create_directories(direct_key_root);

    rk::RpfBuilder direct_child;
    direct_child.add_file("stream/nested_texture.ytd", gen9);
    direct_child.add_file("scripts/preserved.ysc", gen9);
    rk::RpfBuilder direct_root;
    direct_root.add_file("stream/top_texture.ytd", gen9);
    direct_root.add_file("stream/model.ydr", gen9_ydr);
    direct_root.add_file("stream/models.ydd", gen9_ydd);
    direct_root.add_file("stream/vehicle.yft", gen9_yft);
    direct_root.add_file("effects/particles.ypt", gen9_ypt);
    direct_root.add_file("collision/ground.ybn", gen_ybn);
    direct_root.add_file("maps/test.ymap", gen_ymap);
    direct_root.add_file("maps/test.ytyp", gen_ytyp);
    direct_root.add_file("stream/legacy_model.ydr", legacy_ydr_fixture);
    direct_root.add_file("misc/keep.dat", untouched_data);
    const std::vector<std::uint8_t> compressed_payload(32u * 1024u, static_cast<std::uint8_t>('Q'));
    direct_root.add_binary("misc/compressed.bin", compressed_payload, true);
    direct_root.add_binary("misc/empty.bin", {}, false);
    const auto case_sensitive_payload = bytes_of("case-sensitive RPF path");
    direct_root.add_file("MiXeD/Case/keep.bin", case_sensitive_payload);
    direct_root.add_archive("dlcpacks/test/dlc.rpf", std::move(direct_child));
    rk::RpfWriteOptions direct_options;
    direct_options.encryption = rk::RpfEncryption::Open;
    direct_options.name = "direct_source.rpf";
    direct_root.save(direct_source_path.string(), direct_options);

    const rk::Rpf direct_source = rk::Rpf::open(
        direct_source_path.string(), std::make_shared<rk::RpfKeys>());
    const auto source_ysc_location = direct_source.resolve(
        "dlcpacks/test/dlc.rpf/scripts/preserved.ysc");
    require(source_ysc_location.archive && source_ysc_location.entry >= 0,
            "source nested .ysc resource must resolve");
    const auto source_ysc_raw = source_ysc_location.archive->extract_raw(source_ysc_location.entry);

    const std::string direct_report = gta_asset_converter::convert_rpf_archive_to_open_rpf7(
        direct_source_path, direct_output_path, direct_key_root);
    require(direct_report.find("RPF7 conversion complete") == 0,
            "direct archive pipeline must report completion");
    require(direct_report.find("Nested RPFs rebuilt: 1 of 1") != std::string::npos,
            "direct archive pipeline must rebuild its nested RPF");
    require(direct_report.find("Pass-through payloads verified byte-for-byte: binary=") != std::string::npos &&
            direct_report.find("resource streams=") != std::string::npos,
            "direct archive report must confirm byte-level pass-through payload verification");

    const rk::Rpf direct_output = rk::Rpf::open(
        direct_output_path.string(), std::make_shared<rk::RpfKeys>());
    require(direct_output.encryption() == rk::RpfEncryption::Open,
            "direct archive output must be OPEN");
    const auto output_resource_version = [&](const std::string& path) -> std::uint32_t {
        const int index = direct_output.find(path);
        require(index >= 0, "direct archive output must retain a supported resource path");
        return rk::rsc::Resource::load(direct_output.extract(index)).version;
    };
    require(output_resource_version("stream/top_texture.ytd") == 13,
            "direct archive output must contain Legacy top-level YTD");
    require(output_resource_version("stream/model.ydr") == 165,
            "direct archive output must convert YDR");
    require(output_resource_version("stream/models.ydd") == 165,
            "direct archive output must convert YDD");
    require(output_resource_version("stream/vehicle.yft") == 162,
            "direct archive output must convert YFT");
    require(output_resource_version("effects/particles.ypt") == 68,
            "direct archive output must convert YPT");
    const int ybn_index = direct_output.find("collision/ground.ybn");
    const int ymap_index = direct_output.find("maps/test.ymap");
    const int ytyp_index = direct_output.find("maps/test.ytyp");
    require(ybn_index >= 0 && direct_output.extract(ybn_index) == gen_ybn,
            "direct archive output must validate and preserve YBN bytes");
    require(ymap_index >= 0 && direct_output.extract(ymap_index) == gen_ymap,
            "direct archive output must validate and preserve YMAP bytes");
    require(ytyp_index >= 0 && direct_output.extract(ytyp_index) == gen_ytyp,
            "direct archive output must validate and preserve YTYP bytes");
    const int legacy_model_index = direct_output.find("stream/legacy_model.ydr");
    require(legacy_model_index >= 0 && direct_output.extract(legacy_model_index) == legacy_ydr_fixture,
            "direct archive output must preserve an already-Legacy YDR byte-for-byte");
    const auto nested_texture_location = direct_output.resolve(
        "dlcpacks/test/dlc.rpf/stream/nested_texture.ytd");
    require(nested_texture_location.archive && nested_texture_location.entry >= 0 &&
            rk::rsc::Resource::load(nested_texture_location.archive->extract(nested_texture_location.entry)).version == 13,
            "direct archive output must contain converted Legacy nested YTD");
    const auto output_ysc_location = direct_output.resolve(
        "dlcpacks/test/dlc.rpf/scripts/preserved.ysc");
    require(output_ysc_location.archive && output_ysc_location.entry >= 0,
            "direct archive output must retain nested .ysc");
    require(output_ysc_location.archive->entries()[static_cast<std::size_t>(output_ysc_location.entry)].encrypted,
            "nested .ysc resource must retain its encrypted entry marker");
    require(output_ysc_location.archive->extract_raw(output_ysc_location.entry) == source_ysc_raw,
            "nested .ysc stored bytes must be preserved byte-for-byte");
    const int keep_index = direct_output.find("misc/keep.dat");
    require(keep_index >= 0 && direct_output.extract(keep_index) == untouched_data,
            "direct archive output must preserve unsupported binary data");
    const int compressed_index = direct_output.find("misc/compressed.bin");
    const int empty_index = direct_output.find("misc/empty.bin");
    require(compressed_index >= 0 && direct_output.extract(compressed_index) == compressed_payload,
            "compressed pass-through binary must survive direct OPEN repacking");
    require(empty_index >= 0 && direct_output.extract(empty_index).empty(),
            "empty pass-through binary must survive direct OPEN repacking");
    bool exact_case_path_found = false;
    for (const auto& entry : direct_output.entries()) {
        if (entry.path == "MiXeD/Case/keep.bin" && entry.is_binary() &&
            entry.size() == case_sensitive_payload.size()) {
            exact_case_path_found = true;
            break;
        }
    }
    const int case_sensitive_index = direct_output.find("mixed/case/keep.bin");
    require(exact_case_path_found && case_sensitive_index >= 0 &&
            direct_output.extract(case_sensitive_index) == case_sensitive_payload,
            "direct archive output must preserve exact path case and binary payload size");
    // The public integrity verifier must detect same-size payload corruption; entry count,
    // path spelling and logical size alone are not sufficient evidence of a valid round trip.
    const fs::path corrupt_binary_path = fs::temp_directory_path() / "gta-rpf-corrupt-binary-output.rpf";
    const fs::path corrupt_resource_path = fs::temp_directory_path() / "gta-rpf-corrupt-resource-output.rpf";
    fs::remove(corrupt_binary_path, ignored);
    fs::remove(corrupt_resource_path, ignored);
    {
        rk::RpfBuilder corrupted = rk::RpfBuilder::from_archive(direct_source, rk::RpfBuilder::Nested::Rewrite);
        auto same_size_corruption = untouched_data;
        require(!same_size_corruption.empty(), "binary corruption fixture must be non-empty");
        same_size_corruption[0] ^= 0x5Au;
        corrupted.add_binary("misc/keep.dat", same_size_corruption, false);
        rk::RpfWriteOptions options;
        options.encryption = rk::RpfEncryption::Open;
        options.name = corrupt_binary_path.filename().string();
        corrupted.save(corrupt_binary_path.string(), options);
    }
    bool binary_corruption_rejected = false;
    try {
        (void)gta_asset_converter::verify_open_rpf7_payload_integrity(direct_source_path, corrupt_binary_path);
    } catch (const std::exception&) {
        binary_corruption_rejected = true;
    }
    require(binary_corruption_rejected,
            "integrity verifier must reject same-size pass-through binary corruption");

    {
        rk::RpfBuilder corrupted = rk::RpfBuilder::from_archive(direct_source, rk::RpfBuilder::Nested::Rewrite);
        auto same_size_corruption = gen_ybn;
        require(same_size_corruption.size() > 20, "resource corruption fixture must contain a payload");
        same_size_corruption.back() ^= 0xA5u;
        corrupted.add_file("collision/ground.ybn", same_size_corruption);
        rk::RpfWriteOptions options;
        options.encryption = rk::RpfEncryption::Open;
        options.name = corrupt_resource_path.filename().string();
        corrupted.save(corrupt_resource_path.string(), options);
    }
    bool resource_corruption_rejected = false;
    try {
        (void)gta_asset_converter::verify_open_rpf7_payload_integrity(direct_source_path, corrupt_resource_path);
    } catch (const std::exception&) {
        resource_corruption_rejected = true;
    }
    require(resource_corruption_rejected,
            "integrity verifier must reject same-size preserved RSC7 stream corruption");

    fs::remove(corrupt_binary_path, ignored);
    fs::remove(corrupt_resource_path, ignored);
    fs::remove_all(direct_key_root, ignored);
    fs::remove(direct_source_path, ignored);
    fs::remove(direct_output_path, ignored);

    // A YPT with Enhanced-only behaviour must abort the archive conversion before
    // any final output is created; a partial OPEN archive would be misleading.
    const fs::path invalid_ypt_source = fs::temp_directory_path() / "gta-rpf-invalid-ypt-source.rpf";
    const fs::path invalid_ypt_output = fs::temp_directory_path() / "gta-rpf-invalid-ypt-output.rpf";
    fs::remove(invalid_ypt_source, ignored);
    fs::remove(invalid_ypt_output, ignored);
    rk::RpfBuilder invalid_ypt_builder;
    invalid_ypt_builder.add_file("effects/unsupported_ptfx.ypt", gen9_ypt_with_gen9_behaviour);
    rk::RpfWriteOptions invalid_ypt_options;
    invalid_ypt_options.encryption = rk::RpfEncryption::Open;
    invalid_ypt_options.name = "invalid_ypt_source.rpf";
    invalid_ypt_builder.save(invalid_ypt_source.string(), invalid_ypt_options);
    bool invalid_ypt_archive_rejected = false;
    try {
        (void)gta_asset_converter::convert_rpf_archive_to_open_rpf7(
            invalid_ypt_source, invalid_ypt_output, direct_key_root);
    } catch (const std::exception&) {
        invalid_ypt_archive_rejected = true;
    }
    require(invalid_ypt_archive_rejected && !fs::exists(invalid_ypt_output),
            "unsupported Enhanced-only YPT behaviour must fail closed without a final RPF");
    fs::remove(invalid_ypt_source, ignored);


    // Read-only DLC metadata validation: setup2.xml chooses the data XML, and
    // content.xml references must resolve inside the selected dlc.rpf.
    const fs::path dlc_test_root = fs::temp_directory_path() / "gta-rpf-dlc-metadata-tests";
    fs::remove_all(dlc_test_root, ignored);
    fs::create_directories(dlc_test_root);
    const fs::path valid_dlc_path = dlc_test_root / "dlc.rpf";
    const std::string setup_xml =
        "<SSetupData><deviceName>dlc_test</deviceName><datFile>content.xml</datFile>"
        "<nameHash>test</nameHash><contentChangeSetGroups><Item><NameHash>GROUP_STARTUP</NameHash>"
        "<ContentChangeSets><Item>test_AUTOGEN</Item></ContentChangeSets></Item></contentChangeSetGroups>"
        "<type>EXTRACONTENT_COMPAT_PACK</type><order value=\"9\" /></SSetupData>";
    const std::string content_xml =
        "<CDataFileMgr__ContentsOfDataFileXml><disabledFiles/><includedXmlFiles/><includedDataFiles/>"
        "<dataFiles><Item><filename>dlc_test:/vehicles.rpf</filename><fileType>RPF_FILE</fileType></Item>"
        "<Item><filename>dlc_test:/vehicle.meta</filename><fileType>VEHICLE_METADATA_FILE</fileType></Item>"
        "</dataFiles><contentChangeSets/></CDataFileMgr__ContentsOfDataFileXml>";
    rk::RpfBuilder vehicle_archive;
    vehicle_archive.add_file("stream/test.yft", gen9_yft);
    rk::RpfBuilder valid_dlc;
    valid_dlc.add_binary("setup2.xml", bytes_of(setup_xml));
    valid_dlc.add_binary("content.xml", bytes_of(content_xml));
    valid_dlc.add_archive("vehicles.rpf", std::move(vehicle_archive));
    valid_dlc.add_binary("vehicle.meta", bytes_of("<CVehicleModelInfo__InitDataList/>"));
    rk::RpfWriteOptions dlc_options;
    dlc_options.encryption = rk::RpfEncryption::Open;
    dlc_options.name = "dlc.rpf";
    valid_dlc.save(valid_dlc_path.string(), dlc_options);
    const fs::path dlc_empty_keys = dlc_test_root / "keys";
    fs::create_directories(dlc_empty_keys);
    const std::string valid_dlc_report =
        gta_asset_converter::inspect_dlc_pack_metadata(valid_dlc_path, dlc_empty_keys);
    require(valid_dlc_report.find("Device name: dlc_test") != std::string::npos,
            "DLC validator must read the setup2 deviceName");
    require(valid_dlc_report.find("Data files declared: 2") != std::string::npos,
            "DLC validator must count content dataFiles");
    require(valid_dlc_report.find("Data files resolved: 2") != std::string::npos,
            "DLC validator must resolve both a nested RPF and a loose metadata file");
    require(valid_dlc_report.find("Missing references: 0") != std::string::npos,
            "valid DLC content references must not be reported missing");

    // Platform DLC entries are stored inside base x64*.rpf archives. Resolve the
    // dlcpacks/<name>/dlc.rpf entry without extracting the base archive or materializing
    // the nested RPF; validate setup2/content through RAGEKIT's file-backed child reader.
    const fs::path platform_base_path = dlc_test_root / "x64w.rpf";
    rk::RpfBuilder platform_base;
    platform_base.add_archive("dlcpacks/mpBeach/dlc.rpf", valid_dlc.clone());
    platform_base.add_binary("dlcpacks/broken/dlc.rpf", bytes_of("not an RPF7 archive"), false);

    // A real AES-marked nested DLC must be reported as key-locked without the
    // matching user-supplied key, then validate when that exact key is imported.
    const fs::path locked_dlc_path = dlc_test_root / "locked_child.rpf";
    const fs::path dlc_aes_keys = dlc_test_root / "aes-keys";
    fs::create_directories(dlc_aes_keys);
    rk::RpfKeys locked_keys;
    locked_keys.aes.assign(32, 0x2A);
    locked_keys.save(dlc_aes_keys.string());
    rk::RpfBuilder locked_dlc;
    locked_dlc.add_binary("setup2.xml", bytes_of(setup_xml));
    locked_dlc.add_binary("content.xml", bytes_of(content_xml));
    rk::RpfWriteOptions locked_dlc_options;
    locked_dlc_options.encryption = rk::RpfEncryption::Aes;
    locked_dlc_options.keys = std::make_shared<rk::RpfKeys>(locked_keys);
    locked_dlc_options.name = "dlc.rpf";
    locked_dlc.save(locked_dlc_path.string(), locked_dlc_options);
    platform_base.add_binary("dlcpacks/locked/dlc.rpf",
        rk::rsc::read_file(locked_dlc_path.string()), false);

    rk::RpfWriteOptions platform_base_options;
    platform_base_options.encryption = rk::RpfEncryption::Open;
    platform_base_options.name = "x64w.rpf";
    platform_base.save(platform_base_path.string(), platform_base_options);

    const std::string platform_found_report =
        gta_asset_converter::inspect_platform_dlc_pack_metadata(
            platform_base_path, "mpBeach", dlc_empty_keys);
    require(platform_found_report.find("PLATFORM_DLC_FOUND\n") == 0,
            "platform DLC resolver must find a nested dlc.rpf in a base x64 archive");
    require(platform_found_report.find("Internal path: dlcpacks/mpBeach/dlc.rpf") != std::string::npos,
            "platform DLC resolver must report the exact nested archive path");
    require(platform_found_report.find("Device name: dlc_test") != std::string::npos &&
            platform_found_report.find("Data files resolved: 2") != std::string::npos,
            "platform DLC resolver must validate setup2.xml and content.xml in the nested pack");

    const std::string platform_missing_report =
        gta_asset_converter::inspect_platform_dlc_pack_metadata(
            platform_base_path, "mpBusiness", dlc_empty_keys);
    require(platform_missing_report.find("PLATFORM_DLC_NOT_FOUND\n") == 0,
            "platform DLC resolver must distinguish a genuinely absent pack from an unreadable archive");

    const std::string platform_invalid_report =
        gta_asset_converter::inspect_platform_dlc_pack_metadata(
            platform_base_path, "broken", dlc_empty_keys);
    require(platform_invalid_report.find("PLATFORM_DLC_FOUND_INVALID\n") == 0,
            "platform DLC resolver must report a named but malformed nested RPF as invalid");

    const std::string platform_locked_report =
        gta_asset_converter::inspect_platform_dlc_pack_metadata(
            platform_base_path, "locked", dlc_empty_keys);
    require(platform_locked_report.find("PLATFORM_DLC_FOUND_LOCKED\n") == 0,
            "platform DLC resolver must distinguish encrypted nested packs from malformed packs");
    const std::string platform_unlocked_report =
        gta_asset_converter::inspect_platform_dlc_pack_metadata(
            platform_base_path, "locked", dlc_aes_keys);
    require(platform_unlocked_report.find("PLATFORM_DLC_FOUND\n") == 0 &&
            platform_unlocked_report.find("Device name: dlc_test") != std::string::npos,
            "platform DLC resolver must validate an encrypted nested pack when its matching AES key is supplied");

    bool unsafe_platform_name_rejected = false;
    try {
        (void)gta_asset_converter::inspect_platform_dlc_pack_metadata(
            platform_base_path, "../escape", dlc_empty_keys);
    } catch (const std::exception&) {
        unsafe_platform_name_rejected = true;
    }
    require(unsafe_platform_name_rejected,
            "platform DLC resolver must reject traversal pack names before archive lookup");

    const fs::path missing_dlc_path = dlc_test_root / "missing_reference.rpf";
    const std::string missing_content_xml =
        "<CDataFileMgr__ContentsOfDataFileXml><dataFiles>"
        "<Item><filename>dlc_test:/missing.rpf</filename><fileType>RPF_FILE</fileType></Item>"
        "</dataFiles><contentChangeSets/></CDataFileMgr__ContentsOfDataFileXml>";
    rk::RpfBuilder missing_dlc;
    missing_dlc.add_binary("setup2.xml", bytes_of(setup_xml));
    missing_dlc.add_binary("content.xml", bytes_of(missing_content_xml));
    missing_dlc.save(missing_dlc_path.string(), dlc_options);
    const std::string missing_report =
        gta_asset_converter::inspect_dlc_pack_metadata(missing_dlc_path, dlc_empty_keys);
    require(missing_report.find("Missing references: 1") != std::string::npos,
            "DLC validator must report missing RPF_FILE references");

    const fs::path unsafe_dlc_path = dlc_test_root / "unsafe_reference.rpf";
    const std::string unsafe_content_xml =
        "<CDataFileMgr__ContentsOfDataFileXml><dataFiles>"
        "<Item><filename>dlc_test:/../escape.rpf</filename><fileType>RPF_FILE</fileType></Item>"
        "</dataFiles><contentChangeSets/></CDataFileMgr__ContentsOfDataFileXml>";
    rk::RpfBuilder unsafe_dlc;
    unsafe_dlc.add_binary("setup2.xml", bytes_of(setup_xml));
    unsafe_dlc.add_binary("content.xml", bytes_of(unsafe_content_xml));
    unsafe_dlc.save(unsafe_dlc_path.string(), dlc_options);
    const std::string unsafe_report =
        gta_asset_converter::inspect_dlc_pack_metadata(unsafe_dlc_path, dlc_empty_keys);
    require(unsafe_report.find("Unsafe references: 1") != std::string::npos,
            "DLC validator must reject traversal paths in content.xml");
    fs::remove_all(dlc_test_root, ignored);


    // Read update.rpf/common/data/dlclist.xml using the pinned RAGEKIT XML and RPF APIs.
    const fs::path dlc_list_root = fs::temp_directory_path() / "gta-rpf-dlclist-tests";
    fs::remove_all(dlc_list_root, ignored);
    fs::create_directories(dlc_list_root);
    fs::create_directories(dlc_empty_keys);
    const fs::path update_rpf_path = dlc_list_root / "update.rpf";
    const std::string dlclist_xml =
        "<SMandatoryPacksData><Paths>"
        "<Item>dlcpacks:\\testPack\\</Item>"
        "<Item>platform:/dlcPacks/mpBeach/</Item>"
        "<Item>dlcpacks:/../escape/</Item>"
        "<Item>custom:/unknown/</Item>"
        "</Paths></SMandatoryPacksData>";
    rk::RpfBuilder update_builder;
    update_builder.add_binary("common/data/dlclist.xml", bytes_of(dlclist_xml));
    rk::RpfWriteOptions update_options;
    update_options.encryption = rk::RpfEncryption::Open;
    update_options.name = "update.rpf";
    update_builder.save(update_rpf_path.string(), update_options);
    const std::string dlc_list_report =
        gta_asset_converter::read_dlc_list_entries(update_rpf_path, dlc_empty_keys);
    require(dlc_list_report.find("DLC_LIST_V1") == 0,
            "dlclist reader must return the versioned list format");
    require(dlc_list_report.find("DLC|testPack") != std::string::npos,
            "dlclist reader must normalize Windows separators for dlcpacks entries");
    require(dlc_list_report.find("PLATFORM|mpBeach") != std::string::npos,
            "dlclist reader must identify platform DLC entries separately");
    require(dlc_list_report.find("UNSAFE|3") != std::string::npos,
            "dlclist reader must reject traversal pack paths");
    require(dlc_list_report.find("UNSUPPORTED|4") != std::string::npos,
            "dlclist reader must report unknown mount schemes without guessing");

    const fs::path missing_dlclist_path = dlc_list_root / "missing_dlclist.rpf";
    rk::RpfBuilder missing_dlclist_builder;
    missing_dlclist_builder.add_binary("common/data/not_dlclist.xml", bytes_of("<Paths/>"));
    missing_dlclist_builder.save(missing_dlclist_path.string(), update_options);
    bool missing_dlclist_rejected = false;
    try {
        (void)gta_asset_converter::read_dlc_list_entries(missing_dlclist_path, dlc_empty_keys);
    } catch (const std::exception&) {
        missing_dlclist_rejected = true;
    }
    require(missing_dlclist_rejected,
            "dlclist reader must fail when common/data/dlclist.xml is absent");
    fs::remove_all(dlc_list_root, ignored);

    // RAGEKIT's pinned Enhanced notes specify that these resource families keep
    // the same layouts in Legacy and Enhanced. Validate them with their actual
    // readers, preserve their source bytes, and include them in a real RPF7
    // archive round-trip (not just an extension/byte-copy test).
    rk::ClipDictionary clip_dictionary;
    clip_dictionary.root = std::make_shared<rk::RscBlock>();
    clip_dictionary.root->kind = static_cast<std::uint16_t>(rk::ClipKind::Dictionary);
    clip_dictionary.root->bytes.resize(0x40);
    const auto ycd = rk::write_ycd(clip_dictionary);

    rk::PathNodes path_nodes;
    const auto ynd = rk::write_ynd(path_nodes);
    rk::VehicleRecording vehicle_recording;
    const auto yvr = rk::write_yvr(vehicle_recording);
    rk::WaypointRecording waypoint_recording;
    const auto ywr = rk::write_ywr(waypoint_recording);
    rk::ClothDictionary cloth_dictionary;
    const auto yld = rk::write_yld(cloth_dictionary);

    rk::ExpressionDictionary expression_dictionary;
    expression_dictionary.root = std::make_shared<rk::RscBlock>();
    expression_dictionary.root->kind = static_cast<std::uint16_t>(rk::ExpressionKind::Dictionary);
    expression_dictionary.root->bytes.resize(0x40);
    const auto yed = rk::write_yed(expression_dictionary);

    rk::FrameFilterDictionary frame_filter_dictionary;
    frame_filter_dictionary.root = std::make_shared<rk::RscBlock>();
    frame_filter_dictionary.root->kind = static_cast<std::uint16_t>(rk::ExpressionKind::FilterDictionary);
    frame_filter_dictionary.root->bytes.resize(0x40);
    const auto yfd = rk::write_yfd(frame_filter_dictionary);

    rk::BoundDictionary bound_dictionary;
    const auto ybd = rk::write_ybd(bound_dictionary);
    rk::NavMesh navmesh;
    const auto ynv = rk::write_ynv(navmesh);

    gta_asset_converter::ConversionStats edition_independent_stats;
    auto require_validated_and_preserved = [&](const std::string& extension,
                                               const std::vector<std::uint8_t>& fixture) {
        const auto output = gta_asset_converter::convert_asset_bytes(
            extension, fixture, edition_independent_stats);
        require(output == fixture, (extension + " must be validated and preserved byte-for-byte").c_str());
    };
    require_validated_and_preserved("ycd", ycd);
    require_validated_and_preserved("ynd", ynd);
    require_validated_and_preserved("yvr", yvr);
    require_validated_and_preserved("ywr", ywr);
    require_validated_and_preserved("yld", yld);
    require_validated_and_preserved("yed", yed);
    require_validated_and_preserved("yfd", yfd);
    require_validated_and_preserved("ybd", ybd);
    require_validated_and_preserved("ynv", ynv);
    require(edition_independent_stats.validated_ycd == 1 &&
            edition_independent_stats.validated_ynd == 1 &&
            edition_independent_stats.validated_yvr == 1 &&
            edition_independent_stats.validated_ywr == 1 &&
            edition_independent_stats.validated_yld == 1 &&
            edition_independent_stats.validated_yed == 1 &&
            edition_independent_stats.validated_yfd == 1 &&
            edition_independent_stats.validated_ybd == 1 &&
            edition_independent_stats.validated_ynv == 1,
            "all added edition-independent resource validation counters must increment exactly once");

    bool malformed_ycd_rejected = false;
    try {
        (void)gta_asset_converter::convert_asset_bytes("ycd", bytes_of("not an RSC7 resource"), edition_independent_stats);
    } catch (const std::exception&) {
        malformed_ycd_rejected = true;
    }
    require(malformed_ycd_rejected, "malformed YCD resources must fail closed instead of being preserved as validated");

    const fs::path extra_resource_root = fs::temp_directory_path() / "gta-rpf-edition-independent-roundtrip";
    const fs::path extra_keys = extra_resource_root / "keys";
    fs::remove_all(extra_resource_root, ignored);
    fs::create_directories(extra_keys);
    const fs::path extra_source_path = extra_resource_root / "source.rpf";
    const fs::path extra_output_path = extra_resource_root / "android.rpf";
    const std::vector<std::pair<std::string, std::vector<std::uint8_t>>> extra_fixtures = {
        {"edition_independent/animations.ycd", ycd},
        {"edition_independent/pathnodes.ynd", ynd},
        {"edition_independent/vehicle.yvr", yvr},
        {"edition_independent/waypoints.ywr", ywr},
        {"edition_independent/cloth.yld", yld},
        {"edition_independent/expressions.yed", yed},
        {"edition_independent/filters.yfd", yfd},
        {"edition_independent/bounds.ybd", ybd},
        {"edition_independent/navmesh.ynv", ynv},
    };
    rk::RpfBuilder extra_builder;
    for (const auto& [name, bytes] : extra_fixtures) extra_builder.add_resource(name, bytes);
    rk::RpfWriteOptions extra_options;
    extra_options.encryption = rk::RpfEncryption::Open;
    extra_options.name = "source.rpf";
    extra_builder.save(extra_source_path.string(), extra_options);
    const std::string extra_report = gta_asset_converter::convert_rpf_archive_to_open_rpf7(
        extra_source_path, extra_output_path, extra_keys);
    require(extra_report.find("RPF7 conversion complete") == 0,
            "RPF7 conversion must accept the newly validated edition-independent resource families");
    auto extra_output = rk::Rpf::open(extra_output_path.string(),
        std::make_shared<rk::RpfKeys>(rk::RpfKeys::load(extra_keys.string())));
    for (const auto& [name, bytes] : extra_fixtures) {
        const int index = extra_output.find(name);
        const std::string missing_message = "rebuilt RPF7 must retain " + name;
        require(index >= 0, missing_message.c_str());
        const std::string bytes_message = "rebuilt RPF7 must preserve the original edition-independent bytes for " + name;
        require(extra_output.extract(index) == bytes, bytes_message.c_str());
    }
    fs::remove_all(extra_resource_root, ignored);


    // Binary audio and metadata formats are validated with the pinned RAGEKIT readers,
    // then preserved byte-for-byte. AWC uses only the user-imported AWC key; no key is
    // generated or bundled. Enhanced codecs outside PCM/IMA-ADPCM remain opaque.
    const fs::path audio_test_root = fs::temp_directory_path() / "gta-rpf-audio-metadata-tests";
    const fs::path audio_keys_dir = audio_test_root / "keys";
    fs::remove_all(audio_test_root, ignored);
    fs::create_directories(audio_keys_dir);

    rk::RpfKeys audio_key_material;
    audio_key_material.awc.assign(16, 0x5A);
    const fs::path awc_key_path = audio_keys_dir / "gtav_awc_key.dat";
    {
        std::ofstream key_out(awc_key_path, std::ios::binary | std::ios::trunc);
        key_out.write(reinterpret_cast<const char*>(audio_key_material.awc.data()),
                      static_cast<std::streamsize>(audio_key_material.awc.size()));
        require(static_cast<bool>(key_out), "synthetic AWC key fixture must be written");
    }
    const auto audio_keys = std::make_shared<rk::RpfKeys>(audio_key_material);
    const auto no_audio_keys = std::make_shared<rk::RpfKeys>();

    rk::AwcSoundSource sound;
    sound.name = "converter_test_tone";
    sound.sample_rate = 22050;
    sound.pcm.assign(1024, 1234);
    sound.codec = 0;
    sound.loop_point = -1;
    const auto awc_plain = rk::write_awc(rk::make_awc({sound}));
    rk::AwcFile encrypted_awc_model = rk::read_awc(awc_plain, audio_keys);
    encrypted_awc_model.whole_file_encrypted = true;
    encrypted_awc_model.flags = 0xFF09;
    const auto awc_encrypted = rk::write_awc(encrypted_awc_model, audio_keys);

    gta_asset_converter::ConversionStats binary_stats;
    const auto awc_plain_checked = gta_asset_converter::convert_asset_bytes(
        "awc", awc_plain, binary_stats, no_audio_keys);
    require(awc_plain_checked == awc_plain && binary_stats.validated_awc == 1 &&
            binary_stats.decoded_awc_streams == 1,
            "plain AWC must parse, decode PCM and preserve the original bytes");
    bool encrypted_awc_without_key_rejected = false;
    try {
        (void)gta_asset_converter::convert_asset_bytes("awc", awc_encrypted, binary_stats, no_audio_keys);
    } catch (const std::exception&) {
        encrypted_awc_without_key_rejected = true;
    }
    require(encrypted_awc_without_key_rejected,
            "whole-file encrypted AWC must reject missing legitimate AWC key material");
    const auto awc_encrypted_checked = gta_asset_converter::convert_asset_bytes(
        "awc", awc_encrypted, binary_stats, audio_keys);
    require(awc_encrypted_checked == awc_encrypted && binary_stats.validated_awc == 2,
            "encrypted AWC must decrypt for validation and preserve the exact encrypted source bytes");

    rk::AwcFile opaque_awc_model = rk::read_awc(awc_plain, no_audio_keys);
    for (auto& stream : opaque_awc_model.streams) {
        for (auto& chunk : stream.chunks) {
            if (chunk.type == rk::awc_chunk::Format && chunk.data.size() >= 20) chunk.data[19] = 7;
        }
    }
    const auto awc_opaque = rk::write_awc(opaque_awc_model, no_audio_keys);
    const auto awc_opaque_checked = gta_asset_converter::convert_asset_bytes(
        "awc", awc_opaque, binary_stats, no_audio_keys);
    require(awc_opaque_checked == awc_opaque && binary_stats.validated_awc == 3 &&
            binary_stats.decoded_awc_streams == 2 && binary_stats.opaque_awc_streams == 1,
            "unsupported AWC codec 7 must be listed as opaque and preserved, never transcoded");

    const rk::Rel audio_rel = rk::Rel::make(rk::reltype::Dat54, 1);
    const auto rel_bytes = rk::write_rel(audio_rel);
    const auto gxt2_bytes = rk::write_gxt2([&] {
        rk::TextTable table;
        table.set(rk::joaat_lower("CONVERTER_LABEL"), "Converter test text");
        return table;
    }());
    const auto nametable_bytes = rk::write_nametable({"door_open", "door_close"});
    rk::Pso pso_fixture;
    pso_fixture.sections.push_back({rk::kPsoPSIN, std::vector<std::uint8_t>(8, 'p'), false});
    const auto pso_bytes = rk::write_pso(pso_fixture);
    rk::RbfNode rbf_fixture;
    rbf_fixture.name = "root";
    rk::RbfNode rbf_value;
    rbf_value.kind = rk::RbfNode::Kind::Value;
    rbf_value.name = "count";
    rbf_value.value = rk::RbfValue::of(std::uint32_t(3));
    rbf_fixture.children.push_back(rbf_value);
    const auto rbf_bytes = rk::write_rbf(rbf_fixture);

    auto require_binary_roundtrip = [&](const std::string& extension,
                                        const std::vector<std::uint8_t>& fixture) {
        const auto output = gta_asset_converter::convert_asset_bytes(
            extension, fixture, binary_stats, audio_keys);
        require(output == fixture, (extension + " must be validated and preserved byte-for-byte").c_str());
    };
    require_binary_roundtrip("rel", rel_bytes);
    const std::vector<std::pair<std::string, std::uint32_t>> dat_fixtures = {
        {"dat4", rk::reltype::Dat4},
        {"dat10", rk::reltype::Dat10},
        {"dat15", rk::reltype::Dat15},
        {"dat16", rk::reltype::Dat16},
        {"dat22", rk::reltype::Dat22},
        {"dat54", rk::reltype::Dat54},
        {"dat149", rk::reltype::Dat149},
        {"dat150", rk::reltype::Dat150},
        {"dat151", rk::reltype::Dat151},
    };
    for (const auto& [extension, type] : dat_fixtures) {
        const auto fixture = rk::write_rel(rk::Rel::make(type, 1));
        require_binary_roundtrip(extension, fixture);
    }
    require_binary_roundtrip("gxt2", gxt2_bytes);
    require_binary_roundtrip("nametable", nametable_bytes);
    require_binary_roundtrip("ymt", pso_bytes);
    require_binary_roundtrip("ymt", rbf_bytes);
    require_binary_roundtrip("ypdb", pso_bytes);
    require_binary_roundtrip("pso", pso_bytes);
    require_binary_roundtrip("cut", pso_bytes);
    require_binary_roundtrip("ymf", rbf_bytes);
    require_binary_roundtrip("ymf", pso_bytes);
    require(binary_stats.validated_rel == 10 && binary_stats.validated_gxt2 == 1 &&
            binary_stats.validated_nametable == 1 && binary_stats.validated_pso == 5 &&
            binary_stats.validated_rbf == 2,
            "REL/DAT, GXT2, PSO, RBF and nametable counters must report each validated file");

    bool wrong_dat_type_rejected = false;
    try {
        (void)gta_asset_converter::convert_asset_bytes("dat151", rel_bytes, binary_stats, audio_keys);
    } catch (const std::exception&) {
        wrong_dat_type_rejected = true;
    }
    require(wrong_dat_type_rejected, "a DAT extension whose embedded REL type disagrees must be rejected");
    bool malformed_rel_rejected = false;
    try {
        (void)gta_asset_converter::convert_asset_bytes("rel", bytes_of("bad REL"), binary_stats, audio_keys);
    } catch (const std::exception&) {
        malformed_rel_rejected = true;
    }
    require(malformed_rel_rejected, "malformed REL must fail closed");
    const auto unknown_ymt = bytes_of("unrecognized ymt payload");
    require(gta_asset_converter::convert_asset_bytes("ymt", unknown_ymt, binary_stats, audio_keys) == unknown_ymt,
            "unknown YMT containers must be preserved but not misreported as PSO/RBF");

    // Exercise the same validators through an actual RPF7 archive. Encrypted AWC is
    // validated with the imported key, while all binary entries stay source-backed in
    // the OPEN output and must extract to the exact original bytes.
    const fs::path audio_source_path = audio_test_root / "source.rpf";
    const fs::path audio_output_path = audio_test_root / "android.rpf";
    rk::RpfBuilder audio_builder;
    const std::vector<std::pair<std::string, std::vector<std::uint8_t>>> audio_fixtures = {
        {"audio/plain.awc", awc_plain},
        {"audio/encrypted.awc", awc_encrypted},
        {"audio/opaque_codec.awc", awc_opaque},
        {"audio/game.rel", rel_bytes},
        {"audio/game.dat54", rel_bytes},
        {"audio/names.nametable", nametable_bytes},
        {"text/american.gxt2", gxt2_bytes},
        {"metadata/creature.ymt", rbf_bytes},
        {"metadata/manifest.ymf", pso_bytes},
        {"metadata/poses.ypdb", pso_bytes},
        {"metadata/sample.pso", pso_bytes},
        {"metadata/cutscene.cut", pso_bytes},
    };
    for (const auto& [name, bytes] : audio_fixtures) audio_builder.add_binary(name, bytes);
    rk::RpfWriteOptions audio_options;
    audio_options.encryption = rk::RpfEncryption::Open;
    audio_options.name = "source.rpf";
    audio_builder.save(audio_source_path.string(), audio_options);

    const std::string audio_report = gta_asset_converter::convert_rpf_archive_to_open_rpf7(
        audio_source_path, audio_output_path, audio_keys_dir);
    require(audio_report.find("RPF7 conversion complete") == 0 &&
            audio_report.find("AWC=3") != std::string::npos &&
            audio_report.find("REL/DAT=2") != std::string::npos,
            "RPF7 conversion report must distinguish validated audio/metadata formats");
    const rk::Rpf audio_output = rk::Rpf::open(audio_output_path.string(), no_audio_keys);
    for (const auto& [name, bytes] : audio_fixtures) {
        const int index = audio_output.find(name);
        const std::string missing = "RPF7 output must retain audio/metadata entry " + name;
        require(index >= 0, missing.c_str());
        const std::string mismatch = "RPF7 output must preserve exact audio/metadata bytes for " + name;
        require(audio_output.extract(index) == bytes, mismatch.c_str());
    }
    fs::remove_all(audio_test_root, ignored);

    std::cout << "Gen9 conversion, audio/metadata validation, keyed extraction, nested RPF7 and DLC metadata tests passed\n";
    return 0;
}
