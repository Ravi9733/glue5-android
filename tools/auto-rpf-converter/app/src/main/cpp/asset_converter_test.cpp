#include "asset_converter.h"
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <vector>
#include "ragekit/ragekit.h"

static void require(bool condition, const char* message) {
    if (!condition) throw std::runtime_error(message);
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

    std::cout << "Gen9 YTD -> Legacy YTD conversion and directory smoke tests passed\n";
    return 0;
}
