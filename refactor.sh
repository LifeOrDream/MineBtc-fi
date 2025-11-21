#!/bin/bash
find programs/minebtc/src -name "*.rs" -print0 | xargs -0 sed -i ''     -e 's/DogeBtc/MineBtc/g'     -e 's/dogebtc/minebtc/g'     -e 's/dbtc/minebtc/g'     -e 's/DBTC/MINEBTC/g'     -e 's/doge_btc/mine_btc/g'     -e 's/DOGE_BTC/MINE_BTC/g'     -e 's/DOGEBTC/minebtc/g'     -e 's/mdoge/minebtc/g'     -e 's/moon-doge/mine-btc/g'     -e 's/minebtc/minebtc/g'
