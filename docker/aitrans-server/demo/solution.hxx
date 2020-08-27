#ifndef SOLUTION_H_
#define SOLUTION_H_
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <iostream>
#include <map>
using namespace std;
static map<string, uint64_t> your_parameter;
extern "C" {
    struct AckInfo {
        char *event_type;
        uint64_t rtt;
        uint64_t bytes_in_flight;
    };
    struct Blocks {
        uint64_t *blocks_id, *blocks_deadline, *blocks_priority,
            *blocks_create_time, *blocks_size;
        uint64_t block_num;
    };
    void SolutionInit();
    uint64_t SolutionSelectPacket(struct Blocks blocks, uint64_t current_time);
    void SolutionCcTrigger(AckInfo* ack_infos, uint64_t ack_num, uint64_t *congestion_window, uint64_t *pacing_rate);
}

#endif /* SOLUTION_H_ */