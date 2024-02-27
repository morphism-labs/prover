#!/bin/bash

block_number=0x2

while true; do
    data='{"jsonrpc": "2.0", "method": "eth_getBlockTransactionCountByNumber", "params": ["'$block_number'"], "id": 1}'
    
    response=$(curl -s -X POST -H "Content-Type: application/json" -d "$data" http://localhost:6688/)
    
    tx_count=$(echo $response | grep -o '"result":"[^"]*"' | awk -F '"' '{print $4}')
    
    if [ "$tx_count" != "0x0" ]; then
        echo "Block $block_number has $tx_count transactions."
        break
    else
        echo "Block $block_number has no transactions."
    fi
    
    block_number=$(printf "0x%x" $(($block_number + 1)))
done
