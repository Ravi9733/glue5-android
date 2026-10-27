#include "CommandRingBridge.h"

#include <cstdint>
#include <limits>

namespace mugoat::vkhost {
namespace {
constexpr uint32_t kHeaderBytes = 64;
constexpr uint32_t kPacketHeaderBytes = 8;
constexpr uint32_t kWrapOpcode = 1;
}

CommandRingReader::CommandRingReader(const void* memory, size_t memory_size,
                                     size_t ring_base, uint32_t tail,
                                     uint32_t head, uint32_t ring_size)
    : memory_(static_cast<const uint8_t*>(memory)),
      memory_size_(memory_size),
      ring_base_(ring_base),
      tail_(tail),
      head_(head),
      ring_size_(ring_size) {
  valid_ = memory_ != nullptr &&
      (reinterpret_cast<uintptr_t>(memory_) & 3U) == 0 &&
      (ring_base_ & 3U) == 0 && (ring_size_ & 3U) == 0 &&
      ring_base_ <= memory_size_ &&
      ring_size_ <= memory_size_ - ring_base_ &&
      ring_size_ >= kHeaderBytes + kPacketHeaderBytes &&
      tail_ >= kHeaderBytes && tail_ < ring_size_ &&
      head_ >= kHeaderBytes && head_ <= ring_size_ &&
      (tail_ & 3U) == 0 && (head_ & 3U) == 0;
}

PacketStatus CommandRingReader::peek(CommandPacket* out) const {
  if (out == nullptr) return PacketStatus::Invalid;
  *out = {};
  if (!valid_) return PacketStatus::Invalid;
  if (tail_ == head_) return PacketStatus::Empty;
  if (tail_ > ring_size_ - kPacketHeaderBytes) return PacketStatus::Invalid;
  if (tail_ < head_ && head_ - tail_ < kPacketHeaderBytes) return PacketStatus::Invalid;

  const auto* words = reinterpret_cast<const uint32_t*>(
      memory_ + ring_base_ + tail_);
  const uint32_t opcode = words[0];
  const uint32_t payload_words = words[1];
  if (opcode == kWrapOpcode) return PacketStatus::Wrap;

  const uint64_t available_words = (ring_size_ - tail_ - kPacketHeaderBytes) / 4U;
  if (payload_words > available_words) return PacketStatus::Invalid;
  const uint64_t end64 = static_cast<uint64_t>(tail_) + kPacketHeaderBytes +
                         static_cast<uint64_t>(payload_words) * 4U;
  if (end64 > std::numeric_limits<uint32_t>::max()) return PacketStatus::Invalid;
  const auto end = static_cast<uint32_t>(end64);
  if (tail_ < head_ && end > head_) return PacketStatus::Invalid;

  out->opcode = opcode;
  out->payload_words = payload_words;
  out->offset = tail_;
  out->end_offset = end;
  out->payload = words + 2;
  return PacketStatus::Ready;
}

bool CommandRingReader::consumeWrap() {
  if (!valid_ || tail_ > ring_size_ - kPacketHeaderBytes) return false;
  const auto* words = reinterpret_cast<const uint32_t*>(memory_ + ring_base_ + tail_);
  if (words[0] != kWrapOpcode) return false;
  // WRAP is a marker, not a regular packet. The next readable packet begins
  // at the first byte after the fixed ring header.
  tail_ = kHeaderBytes;
  return true;
}

bool CommandRingReader::consume(const CommandPacket& packet) {
  if (!valid_ || packet.offset != tail_ ||
      packet.end_offset != static_cast<uint64_t>(tail_) +
                               kPacketHeaderBytes +
                               static_cast<uint64_t>(packet.payload_words) * 4U ||
      packet.end_offset > ring_size_ ||
      (tail_ < head_ && packet.end_offset > head_) ||
      packet.payload != reinterpret_cast<const uint32_t*>(
                            memory_ + ring_base_ + tail_ + kPacketHeaderBytes)) {
    return false;
  }
  tail_ = packet.end_offset;
  return true;
}

}  // namespace mugoat::vkhost
