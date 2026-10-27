#include "CommandRingBridge.h"

#include <cassert>
#include <cstdint>
#include <vector>

using mugoat::vkhost::CommandPacket;
using mugoat::vkhost::CommandRingReader;
using mugoat::vkhost::PacketStatus;

namespace {
constexpr uint32_t kRingBytes = 128;
constexpr uint32_t kData = 64;

std::vector<uint32_t> memory() { return std::vector<uint32_t>(kRingBytes / 4, 0); }

void empty_ring_is_empty() {
  auto mem = memory();
  CommandRingReader reader(mem.data(), mem.size() * 4, 0, kData, kData, kRingBytes);
  CommandPacket packet;
  assert(reader.peek(&packet) == PacketStatus::Empty);
}

void valid_packet_can_be_consumed() {
  auto mem = memory();
  mem[16] = 10;  // opcode
  mem[17] = 1;   // payload words
  mem[18] = 0x12345678;
  CommandRingReader reader(mem.data(), mem.size() * 4, 0, kData, 76, kRingBytes);
  CommandPacket packet;
  assert(reader.peek(&packet) == PacketStatus::Ready);
  assert(packet.opcode == 10 && packet.payload_words == 1);
  assert(packet.payload[0] == 0x12345678);
  assert(reader.consume(packet));
  assert(reader.tail() == 76);
}

void truncated_packet_is_rejected() {
  auto mem = memory();
  mem[16] = 10;
  mem[17] = 2;
  CommandRingReader reader(mem.data(), mem.size() * 4, 0, kData, 72, kRingBytes);
  CommandPacket packet;
  assert(reader.peek(&packet) == PacketStatus::Invalid);
}

void oversized_packet_is_rejected() {
  auto mem = memory();
  mem[16] = 10;
  mem[17] = 0xffffffffU;
  CommandRingReader reader(mem.data(), mem.size() * 4, 0, kData, 120, kRingBytes);
  CommandPacket packet;
  assert(reader.peek(&packet) == PacketStatus::Invalid);
}

void wrap_is_accepted_only_at_ring_end() {
  auto mem = memory();
  mem[25] = 1;  // WRAP at byte offset 100
  CommandRingReader wrapped(mem.data(), mem.size() * 4, 0, 100, 80, kRingBytes);
  CommandPacket packet;
  assert(wrapped.peek(&packet) == PacketStatus::Wrap);
  assert(wrapped.consumeWrap());
  assert(wrapped.tail() == kData);

  auto invalid_mem = memory();
  invalid_mem[16] = 1;
  CommandRingReader invalid(invalid_mem.data(), invalid_mem.size() * 4, 0,
                            kData, 100, kRingBytes);
  assert(invalid.peek(&packet) == PacketStatus::Invalid);
}

void invalid_mapping_is_rejected() {
  auto mem = memory();
  CommandRingReader reader(mem.data(), mem.size() * 4, 4, kData, 80, kRingBytes);
  CommandPacket packet;
  assert(reader.peek(&packet) == PacketStatus::Invalid);
}

void stale_or_forged_packet_cannot_advance_tail() {
  auto mem = memory();
  mem[16] = 10;
  mem[17] = 1;
  mem[18] = 0x12345678;
  CommandRingReader reader(mem.data(), mem.size() * 4, 0, kData, 76, kRingBytes);
  CommandPacket packet;
  assert(reader.peek(&packet) == PacketStatus::Ready);

  CommandPacket forged = packet;
  forged.end_offset = 80;
  assert(!reader.consume(forged));
  assert(reader.tail() == kData);

  forged = packet;
  forged.payload_words = 2;
  assert(!reader.consume(forged));
  assert(reader.tail() == kData);

  assert(reader.consume(packet));
  assert(reader.tail() == 76);
  assert(!reader.consume(packet));
  assert(reader.tail() == 76);
}

void wrapped_packet_must_not_be_consumed_as_regular_data() {
  auto mem = memory();
  mem[25] = 1;  // WRAP at byte offset 100
  CommandRingReader reader(mem.data(), mem.size() * 4, 0, 100, 80, kRingBytes);
  CommandPacket packet;
  assert(reader.peek(&packet) == PacketStatus::Wrap);
  assert(!reader.consume(packet));
  assert(reader.tail() == 100);
  assert(reader.consumeWrap());
  assert(reader.tail() == kData);
}
}  // namespace

int main() {
  empty_ring_is_empty();
  valid_packet_can_be_consumed();
  truncated_packet_is_rejected();
  oversized_packet_is_rejected();
  wrap_is_accepted_only_at_ring_end();
  invalid_mapping_is_rejected();
}
