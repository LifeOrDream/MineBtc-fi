/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/minebtc.json`.
 */
export type Minebtc = {
  "address": "FoACK8AbSqy9rPtRccTyqxhgHX9xSLWYnBEfXvDNPX61",
  "metadata": {
    "name": "minebtc",
    "version": "1.0.0",
    "spec": "0.1.0",
    "description": "MineBTC — Faction warfare game on Solana with betting, staking, NFTs, and deflationary tokenomics",
    "repository": "https://github.com/LifeOrDream/MineBtc-fi"
  },
  "instructions": [
    {
      "name": "acceptAuthority",
      "docs": [
        "Accept a proposed authority transfer (step 2). Only the pending authority can call."
      ],
      "discriminator": [
        107,
        86,
        198,
        91,
        33,
        12,
        107,
        160
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "newAuthority",
          "docs": [
            "The new authority (must match pending_authority)"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "addCollectionDelegate",
      "docs": [
        "Add an UpdateDelegate to the collection (admin only)",
        "Allows delegate wallet to sign for marketplace verification without",
        "transferring the update authority (which would break minting)"
      ],
      "discriminator": [
        74,
        132,
        189,
        217,
        153,
        199,
        142,
        201
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastsConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "collection",
          "writable": true
        },
        {
          "name": "collectionAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  108,
                  108,
                  101,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "delegate",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "addFaction",
      "docs": [
        "Add a single faction to the global config (admin only)"
      ],
      "discriminator": [
        69,
        217,
        80,
        143,
        233,
        243,
        254,
        245
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "factionState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "arg",
                "path": "factionName"
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "factionName",
          "type": "string"
        },
        {
          "name": "factionId",
          "type": "u8"
        }
      ]
    },
    {
      "name": "addLpAndBurn",
      "docs": [
        "INSTRUCTION 2b: Add liquidity and burn LP tokens (called after update_rate)",
        "When lp_token_amount > 0: Admin override mode (requires authority signature)",
        "When lp_token_amount = 0: Automatic calculation mode (anyone can call)"
      ],
      "discriminator": [
        242,
        110,
        45,
        83,
        157,
        74,
        181,
        139
      ],
      "accounts": [
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalGameState",
          "docs": [
            "Read-only: provides `current_round_id` so we can snapshot the cycle's",
            "final round when the LP op crosses the war's settle threshold."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "docs": [
            "Mut: this ix writes `cycle_end_round_id` once the lp threshold crosses."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "docs": [
            "Authority (optional - only required when lp_token_amount > 0)"
          ],
          "signer": true,
          "optional": true
        },
        {
          "name": "raydiumProgram",
          "docs": [
            "`raydium_cp_swap::ID` so a malicious program can't receive our CPI",
            "(which signs with the `authority_pda` PDA) and drain program-owned",
            "token accounts via authority_pda's signer privilege."
          ],
          "address": "68NJDT912wd5EuCB3jDabR77gCjM3xgfkJ21hUxgfYJ4"
        },
        {
          "name": "poolState",
          "writable": true
        },
        {
          "name": "authorityPda",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "raydiumAuthority"
        },
        {
          "name": "dbtcVault",
          "writable": true
        },
        {
          "name": "solVault",
          "writable": true
        },
        {
          "name": "dbtcTokenAccount",
          "docs": [
            "MINE_BTC token vault"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "solTokenAccount",
          "docs": [
            "SOL token account for LP addition. **Must be the canonical WSOL ATA",
            "owned by `authority_pda`** — without this binding, a caller could pass",
            "an attacker-owned WSOL account and siphon the earmarked",
            "`buybacks_account.sol_for_pol` through the early-return path (e.g. by",
            "also passing a zero-balance `sol_vault`). With the constraint, any",
            "SOL transferred here is held under `authority_pda`'s signer authority",
            "and is recovered to `buybacks_sol_vault` by the trailing close at the",
            "end of this ix. SnapshotPrice init-if-needs this exact ATA, so it will",
            "exist by the time the first LP burn fires."
          ],
          "writable": true
        },
        {
          "name": "degenbtcMint",
          "writable": true
        },
        {
          "name": "solMint",
          "docs": [
            "— same rationale as `SnapshotPrice::sol_mint`."
          ],
          "writable": true,
          "address": "So11111111111111111111111111111111111111112"
        },
        {
          "name": "lpTokenAccount",
          "writable": true
        },
        {
          "name": "lpMint",
          "writable": true
        },
        {
          "name": "tokenProgram2022",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "buybacksSolVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  98,
                  117,
                  121,
                  98,
                  97,
                  99,
                  107,
                  115,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "buybacksAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  98,
                  117,
                  121,
                  98,
                  97,
                  99,
                  107,
                  115
                ]
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "lpTokenAmount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "addTicketTierConfig",
      "docs": [
        "Add or update ticket tier configs (admin only)",
        "Max 3 ticket tier configs can be set"
      ],
      "discriminator": [
        165,
        68,
        99,
        80,
        164,
        244,
        254,
        42
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastMintConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "ticketTierIndex",
          "type": "u8"
        },
        {
          "name": "ticketValue",
          "type": "u64"
        }
      ]
    },
    {
      "name": "adminMintHashbeast",
      "docs": [
        "Admin function to mint a HashBeast NFT for free to a specified recipient (admin only)",
        "",
        "Allows the admin to mint a HashBeast NFT without payment.",
        "The NFT is minted directly to the specified recipient address.",
        "",
        "# Parameters",
        "- `recipient`: Address that will receive the minted NFT",
        "- `faction_id`: Faction ID the hashbeast belongs to"
      ],
      "discriminator": [
        98,
        180,
        191,
        182,
        53,
        240,
        191,
        170
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastMintConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "recipient",
          "writable": true
        },
        {
          "name": "playerData",
          "docs": [
            "Player data account for the recipient (for ticket distribution)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "arg",
                "path": "recipient"
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset",
          "docs": [
            "Metaplex Core asset (will be created)"
          ],
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "Collection account for the HashBeast. Address-pinned to the official",
            "Core collection recorded in `hashbeast_config.hashbeast_collection`",
            "— without this binding, callers could mint NFT assets outside the",
            "canonical collection, breaking identity / royalties / marketplace",
            "gating. Kept `Option` (rather than required) only to preserve the",
            "existing SDK signature; mint handlers add a runtime `require!(...)`",
            "that the collection is present."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "hashbeastMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "collectionAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  108,
                  108,
                  101,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "recipient",
          "type": "pubkey"
        },
        {
          "name": "factionId",
          "type": "u8"
        },
        {
          "name": "ticketTierIndex",
          "type": "u8"
        }
      ]
    },
    {
      "name": "batchMintHashbeasts",
      "docs": [
        "Batch mint multiple HashBeast (anyone can call, max 10 per transaction)",
        "",
        "Mints multiple HashBeast NFTs in a single transaction.",
        "Each hashbeast uses bonding curve pricing based on the current supply at mint time.",
        "",
        "# Parameters",
        "- `faction_id`: Faction ID all hashbeasts belong to",
        "- `mint_count`: Number of hashbeasts to mint (1-10)",
        "- `ticket_tier_index`: Ticket tier index (0-2)"
      ],
      "discriminator": [
        71,
        33,
        35,
        173,
        22,
        216,
        12,
        66
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastMintConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "multisigWsolAccount",
          "docs": [
            "Multisig WSOL token account (destination for WSOL transfers)",
            "MUST be owned by global_config.fee_recipient (the multisig address)"
          ],
          "writable": true
        },
        {
          "name": "userWsolAccount",
          "docs": [
            "User's WSOL token account (for wrapping SOL to WSOL)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "user"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "wsolMint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "wsolMint"
        },
        {
          "name": "hashbeastCollection",
          "writable": true,
          "optional": true
        },
        {
          "name": "collectionAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  108,
                  108,
                  101,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "user",
          "writable": true,
          "signer": true
        },
        {
          "name": "referrerRewards",
          "docs": [
            "Optional only when the minter has no referrer.",
            "Referred minters must provide the canonical referrer's ReferralRewards PDA."
          ],
          "writable": true,
          "optional": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  102,
                  101,
                  114,
                  114,
                  97,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "player_data.referral_code",
                "account": "playerData"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "factionId",
          "type": "u8"
        },
        {
          "name": "mintCount",
          "type": "u8"
        },
        {
          "name": "ticketTierIndex",
          "type": "u8"
        }
      ]
    },
    {
      "name": "breedHashbeasts",
      "docs": [
        "Breed two hashbeasts to create offspring"
      ],
      "discriminator": [
        205,
        21,
        28,
        59,
        34,
        205,
        182,
        121
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastMintConfig",
          "docs": [
            "account validator under the BPF stack limit."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "docs": [
            "deserializing the full PlayerData account in the generated validator."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "solTreasury",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  45,
                  116,
                  114,
                  101,
                  97,
                  115,
                  117,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "feeRecipient",
          "writable": true
        },
        {
          "name": "floorHistory",
          "docs": [
            "account validator under the BPF stack limit."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  104,
                  105,
                  115,
                  116,
                  111,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcTokenVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "vaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "userTokenAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "user"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "tokenMint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "tokenMint",
          "writable": true
        },
        {
          "name": "momAsset",
          "writable": true
        },
        {
          "name": "momMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "momAsset"
              }
            ]
          }
        },
        {
          "name": "dadAsset",
          "writable": true
        },
        {
          "name": "dadMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "dadAsset"
              }
            ]
          }
        },
        {
          "name": "offspringAsset",
          "writable": true,
          "signer": true
        },
        {
          "name": "offspringMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "offspringAsset"
              }
            ]
          }
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "`hashbeast_config.hashbeast_collection` to keep generated account",
            "validation below the SBF stack ceiling."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "collectionAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  108,
                  108,
                  101,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "tokenProgram2022",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "buyUserListing",
      "docs": [
        "Permissionless wrapper around `degenbtc_market::buy_listing`. Records",
        "a real-demand sale to `SaleHistory` if it qualifies (user-to-user,",
        "listing >= 5 minutes old)."
      ],
      "discriminator": [
        28,
        79,
        242,
        206,
        150,
        136,
        126,
        183
      ],
      "accounts": [
        {
          "name": "buyer",
          "writable": true,
          "signer": true
        },
        {
          "name": "seller",
          "writable": true
        },
        {
          "name": "inventoryPool",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "floorQueue",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  113,
                  117,
                  101,
                  117,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "saleHistory",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  97,
                  108,
                  101,
                  45,
                  104,
                  105,
                  115,
                  116,
                  111,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "marketplaceConfig"
        },
        {
          "name": "marketplaceListing",
          "writable": true
        },
        {
          "name": "hashbeastAsset",
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "writable": true
        },
        {
          "name": "marketplaceEscrow",
          "writable": true
        },
        {
          "name": "feeRecipient",
          "writable": true
        },
        {
          "name": "marketplaceProgram"
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "maxPriceLamports",
          "type": "u64"
        }
      ]
    },
    {
      "name": "cancelAuthorityTransfer",
      "docs": [
        "Cancel a pending authority transfer. Only current authority can call."
      ],
      "discriminator": [
        94,
        131,
        125,
        184,
        183,
        24,
        125,
        229
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "cancelUserListing",
      "docs": [
        "User cancels their own listing; floor queue entry (if any) is removed."
      ],
      "discriminator": [
        207,
        13,
        178,
        91,
        88,
        146,
        161,
        231
      ],
      "accounts": [
        {
          "name": "seller",
          "writable": true,
          "signer": true
        },
        {
          "name": "inventoryPool",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "floorQueue",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  113,
                  117,
                  101,
                  117,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "marketplaceConfig",
          "writable": true
        },
        {
          "name": "marketplaceListing",
          "writable": true
        },
        {
          "name": "hashbeastAsset",
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "writable": true
        },
        {
          "name": "marketplaceEscrow",
          "writable": true
        },
        {
          "name": "marketplaceProgram"
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "claimAutominerRewards",
      "docs": [
        "Claim autominer rewards with auto-reload (keeper instruction)",
        "Uses SOL rewards to add more rounds to autominer, leftover SOL goes to owner"
      ],
      "discriminator": [
        37,
        223,
        191,
        17,
        249,
        154,
        81,
        77
      ],
      "accounts": [
        {
          "name": "autominerVault",
          "docs": [
            "Autominer vault to reload"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              }
            ]
          }
        },
        {
          "name": "autominerCustody",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              }
            ]
          }
        },
        {
          "name": "hodlPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  111,
                  100,
                  108,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "gameSession",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "arg",
                "path": "roundId"
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "solPrizePotVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  106,
                  97,
                  99,
                  107,
                  112,
                  111,
                  116,
                  45,
                  112,
                  111,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "userGameBet",
          "docs": [
            "User game bet account - will be closed"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  98,
                  101,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              },
              {
                "kind": "arg",
                "path": "roundId"
              }
            ]
          }
        },
        {
          "name": "ownerWallet",
          "writable": true
        },
        {
          "name": "caller",
          "docs": [
            "Caller (backend script)"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "hashbeastMetadata",
          "docs": [
            "Optional HashBeastMetadata account for syncing mutation"
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "warState",
          "docs": [
            "Cycle state for the round being claimed. Address pinned via seeds keyed",
            "by `game_session.war_id_when_played`."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "game_session.war_id_when_played",
                "account": "gameSession"
              }
            ]
          }
        },
        {
          "name": "userWarBets",
          "docs": [
            "Per-user, per-cycle bets PDA for the autominer's owner. Seeds pinned",
            "to `autominer_vault.owner` + the cycle id on `game_session`."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              },
              {
                "kind": "account",
                "path": "game_session.war_id_when_played",
                "account": "gameSession"
              }
            ]
          }
        },
        {
          "name": "lootboxQueue",
          "docs": [
            "Country lootbox queue for the autominer owner's home faction."
          ],
          "writable": true
        },
        {
          "name": "lootboxClaim",
          "docs": [
            "handler either verifies the existing program-owned `LootboxClaim` data or",
            "lazily creates it only after a winning loser-roll lands."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  111,
                  116,
                  98,
                  111,
                  120,
                  45,
                  99,
                  108,
                  97,
                  105,
                  109
                ]
              },
              {
                "kind": "account",
                "path": "ownerWallet"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "roundId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "claimFactionTreasuryForFactionWar",
      "docs": [
        "DONE -::- Claim faction treasury rewards for a settled faction_war.",
        "Uses the gameplay-score leaderboard (faction_war final_ranks) -- permissionless."
      ],
      "discriminator": [
        148,
        57,
        179,
        26,
        95,
        218,
        143,
        182
      ],
      "accounts": [
        {
          "name": "taxConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  116,
                  97,
                  120,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "warSettlement",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  101,
                  116,
                  116,
                  108,
                  101,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "degenbtcMint"
        },
        {
          "name": "factionTreasuryVault",
          "writable": true
        },
        {
          "name": "dbtcMining",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "vaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcEmissionVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "withdrawWithheldAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  119,
                  105,
                  116,
                  104,
                  100,
                  114,
                  97,
                  119,
                  45,
                  119,
                  105,
                  116,
                  104,
                  104,
                  101,
                  108,
                  100,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "tokenProgram2022",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        }
      ],
      "args": [
        {
          "name": "warId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "claimLootboxNft",
      "docs": [
        "Permissionless. Delivers a reserved loser-roll hashbeast to the recorded",
        "winner. Signer may be the user or a cranker bot."
      ],
      "discriminator": [
        206,
        200,
        231,
        61,
        32,
        177,
        101,
        3
      ],
      "accounts": [
        {
          "name": "cranker",
          "writable": true,
          "signer": true
        },
        {
          "name": "inventoryPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "inventoryPda",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "lootboxClaim",
          "docs": [
            "Reservation PDA, closed on success. Rent goes to the cranker as the",
            "delivery incentive; the NFT recipient remains fixed by `lootbox_claim.user`."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  111,
                  116,
                  98,
                  111,
                  120,
                  45,
                  99,
                  108,
                  97,
                  105,
                  109
                ]
              },
              {
                "kind": "account",
                "path": "lootbox_claim.user",
                "account": "lootboxClaim"
              }
            ]
          }
        },
        {
          "name": "user",
          "writable": true
        },
        {
          "name": "rebornEntry",
          "docs": [
            "RebornEntry for the dropped asset, closed on success."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  98,
                  111,
                  114,
                  110,
                  45,
                  101,
                  110,
                  116,
                  114,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset",
          "writable": true
        },
        {
          "name": "hashbeastMetadata",
          "docs": [
            "HashBeast metadata, read for rebirth generation emitted to indexers."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "hashbeastConfig",
          "docs": [
            "Read-only config that pins the canonical HashBeast collection."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "Option wrapper is only for mpl-core builder compatibility; handler",
            "requires Some and Anchor address-checks it against HashBeastConfig."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "claimReferralRewards",
      "docs": [
        "Claim referral rewards (SOL earned from referrals)"
      ],
      "discriminator": [
        23,
        112,
        76,
        162,
        157,
        106,
        203,
        246
      ],
      "accounts": [
        {
          "name": "referralRewards",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  102,
                  101,
                  114,
                  114,
                  97,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "authority",
          "docs": [
            "Referrer claiming rewards"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "claimRoundRewards",
      "docs": [
        "Claim rewards for a user after round ends"
      ],
      "discriminator": [
        216,
        184,
        9,
        22,
        131,
        51,
        186,
        140
      ],
      "accounts": [
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "userWallet"
              }
            ]
          }
        },
        {
          "name": "hodlPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  111,
                  100,
                  108,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "gameSession",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "arg",
                "path": "roundId"
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalGameState",
          "docs": [
            "Global game state (for current round entropy)"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "solPrizePotVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  106,
                  97,
                  99,
                  107,
                  112,
                  111,
                  116,
                  45,
                  112,
                  111,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "userGameBet",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  98,
                  101,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "userWallet"
              },
              {
                "kind": "arg",
                "path": "roundId"
              }
            ]
          }
        },
        {
          "name": "userWallet",
          "writable": true
        },
        {
          "name": "caller",
          "docs": [
            "Caller (bot or user themselves)"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "hashbeastMetadata",
          "docs": [
            "Optional HashBeastMetadata account for syncing mutation"
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "warState",
          "docs": [
            "Cycle state for the round being claimed. Address is pinned via seeds",
            "keyed by `game_session.war_id_when_played`."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "game_session.war_id_when_played",
                "account": "gameSession"
              }
            ]
          }
        },
        {
          "name": "userWarBets",
          "docs": [
            "Per-user, per-cycle bets PDA. Address pinned via seeds keyed by the",
            "claiming user + the cycle id stored on `game_session`. Lazily created",
            "by `join_bets` (the first bet of the cycle); claim ixs read+mutate the",
            "already-existing account."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "userWallet"
              },
              {
                "kind": "account",
                "path": "game_session.war_id_when_played",
                "account": "gameSession"
              }
            ]
          }
        },
        {
          "name": "lootboxQueue",
          "docs": [
            "Country lootbox queue for the player's home faction. Read on every",
            "claim; mutated when a losing player's roll wins a slot."
          ],
          "writable": true
        },
        {
          "name": "lootboxClaim",
          "docs": [
            "handler either verifies the existing program-owned `LootboxClaim` data or",
            "lazily creates it only after a winning loser-roll lands."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  111,
                  111,
                  116,
                  98,
                  111,
                  120,
                  45,
                  99,
                  108,
                  97,
                  105,
                  109
                ]
              },
              {
                "kind": "account",
                "path": "userWallet"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "roundId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "claimStakingRewards",
      "docs": [
        "Claim staking rewards: transfers SOL directly, accumulates degenBTC to pending"
      ],
      "discriminator": [
        229,
        141,
        170,
        69,
        111,
        94,
        6,
        72
      ],
      "accounts": [
        {
          "name": "factionState"
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "solRewardsVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "degenbtcMint"
        },
        {
          "name": "userDbtcAccount",
          "docs": [
            "User's degenBTC token account to receive staking rewards"
          ],
          "writable": true
        },
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcTokenVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "dbtcVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "docs": [
            "User claiming rewards (must be player_data.owner)"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "tokenProgram",
          "docs": [
            "Token-2022 program for SPL-22 token operations"
          ],
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        }
      ],
      "args": []
    },
    {
      "name": "claimWarRewards",
      "docs": [
        "User claims their faction-war rewards (closes user_war_bets account)."
      ],
      "discriminator": [
        186,
        177,
        25,
        84,
        70,
        42,
        85,
        26
      ],
      "accounts": [
        {
          "name": "warState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "warSettlement",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  101,
                  116,
                  116,
                  108,
                  101,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "userWarBets",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user_war_bets.owner",
                "account": "userFactionWarBets"
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user_war_bets.owner",
                "account": "userFactionWarBets"
              }
            ]
          }
        },
        {
          "name": "hodlPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  111,
                  100,
                  108,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastMetadata",
          "writable": true,
          "optional": true
        },
        {
          "name": "warSolVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "player",
          "writable": true
        },
        {
          "name": "cranker",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "warId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "crankDistributeTax",
      "docs": [
        "DONE -::- STEP 2: Withdraw total tax from mint and distribute it",
        "Callable by anyone - program-controlled withdraw authority"
      ],
      "discriminator": [
        188,
        72,
        20,
        216,
        153,
        214,
        152,
        184
      ],
      "accounts": [
        {
          "name": "degenbtcMint",
          "writable": true
        },
        {
          "name": "withdrawWithheldAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  119,
                  105,
                  116,
                  104,
                  100,
                  114,
                  97,
                  119,
                  45,
                  119,
                  105,
                  116,
                  104,
                  104,
                  101,
                  108,
                  100,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "withdrawAuthorityTokenAccount",
          "writable": true
        },
        {
          "name": "factionTreasuryVault",
          "writable": true
        },
        {
          "name": "dbtcMining",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "vaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcTokenVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "taxConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  116,
                  97,
                  120,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "caller",
          "writable": true,
          "signer": true
        },
        {
          "name": "tokenProgram2022",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "warId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "crankHarvestFees",
      "docs": [
        "DONE -::- STEP 1: Harvest fees from user token accounts to the mint",
        "Callable by anyone - keeper bot should call this in batches"
      ],
      "discriminator": [
        82,
        205,
        136,
        151,
        201,
        75,
        23,
        79
      ],
      "accounts": [
        {
          "name": "degenbtcMint",
          "writable": true
        },
        {
          "name": "tokenProgram2022",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        }
      ],
      "args": []
    },
    {
      "name": "createHashbeastCollection",
      "docs": [
        "Create HashBeast collection with program PDA as authority (admin only)",
        "",
        "Creates a new Metaplex Core collection for HashBeast NFTs.",
        "The collection's update authority is set to a program-controlled PDA,",
        "allowing the program to mint NFTs from the collection.",
        "Requires HashBeastConfig to be initialized first."
      ],
      "discriminator": [
        45,
        73,
        231,
        84,
        247,
        234,
        188,
        84
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastsConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "collection",
          "writable": true,
          "signer": true
        },
        {
          "name": "collectionAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  108,
                  108,
                  101,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "name",
          "type": "string"
        },
        {
          "name": "uri",
          "type": "string"
        }
      ]
    },
    {
      "name": "depositDbtcTokens",
      "docs": [
        "Deposit degenBTC tokens to the mining vault (anyone can call)",
        "",
        "Allows anyone to deposit degenBTC tokens into the mining vault.",
        "These tokens will be distributed as rewards to stakers over time."
      ],
      "discriminator": [
        132,
        180,
        18,
        63,
        99,
        252,
        68,
        122
      ],
      "accounts": [
        {
          "name": "depositor",
          "writable": true,
          "signer": true
        },
        {
          "name": "depositorTokenAccount",
          "writable": true
        },
        {
          "name": "dbtcTokenVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "dbtcMining",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "tokenMint"
        },
        {
          "name": "tokenProgram",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "distributeSolFees",
      "docs": [
        "Withdraw collected SOL fees from the treasury (anyone can call)",
        "",
        "Withdraws SOL from the treasury and distributes it according to configured percentages:",
        "- Protocol fee percentage",
        "- Buyback percentage (for token buybacks)",
        "- Stakers percentage (distributed to stakers)",
        "",
        "The remaining amount goes to the fee recipient (dev earnings)."
      ],
      "discriminator": [
        237,
        212,
        201,
        211,
        7,
        62,
        123,
        186
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "solTreasury",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  45,
                  116,
                  114,
                  101,
                  97,
                  115,
                  117,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "treasuryWsolAccount",
          "docs": [
            "Treasury's WSOL token account (authority is treasury PDA)",
            "Initialized automatically if it doesn't exist (payer pays for initialization)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "solTreasury"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "wsolMint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "multisigWsolAccount",
          "docs": [
            "Multisig WSOL token account (destination for WSOL transfers)",
            "MUST be owned by global_config.fee_recipient (the multisig address)"
          ],
          "writable": true
        },
        {
          "name": "wsolMint",
          "docs": [
            "caller can't pass a fake mint, drive `treasury_wsol_account` ATA-init at",
            "a different mint and silently divert dev-earnings into an attacker ATA.",
            "(sync_native already would catch this on the SPL side, but constraining",
            "here fails earlier with a clearer error and removes the foot-gun.)"
          ],
          "address": "So11111111111111111111111111111111111111112"
        },
        {
          "name": "buybacksSolVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  98,
                  117,
                  121,
                  98,
                  97,
                  99,
                  107,
                  115,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "inventorySweepVault",
          "docs": [
            "`nft_market_making_pct` slice of distributed SOL fees."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  115,
                  119,
                  101,
                  101,
                  112,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "buybacksAccount",
          "docs": [
            "Buybacks tracking account (required)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  98,
                  117,
                  121,
                  98,
                  97,
                  99,
                  107,
                  115
                ]
              }
            ]
          }
        },
        {
          "name": "payer",
          "docs": [
            "Payer for account initialization (can be anyone calling this keeper function)"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "endRound",
      "docs": [
        "DONE -::- Finalize the current round using scheduled slot-hash entropy."
      ],
      "discriminator": [
        54,
        47,
        1,
        200,
        250,
        6,
        144,
        63
      ],
      "accounts": [
        {
          "name": "gameSession",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "global_game_state.current_round_id",
                "account": "globalGameSate"
              }
            ]
          }
        },
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalGameState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "slotHashes",
          "address": "SysvarS1otHashes111111111111111111111111111"
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "executeAutominerBet",
      "docs": [
        "Execute autominer bet (keeper instruction)"
      ],
      "discriminator": [
        18,
        60,
        67,
        179,
        246,
        26,
        124,
        212
      ],
      "accounts": [
        {
          "name": "autominerVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              }
            ]
          }
        },
        {
          "name": "autominerCustody",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "globalGameState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              }
            ]
          }
        },
        {
          "name": "gameSession",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "arg",
                "path": "currentRoundId"
              }
            ]
          }
        },
        {
          "name": "userGameBet",
          "docs": [
            "UserGameBet PDA for autominer bets (aggregates all bets from this vault for this round)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  98,
                  101,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              },
              {
                "kind": "arg",
                "path": "currentRoundId"
              }
            ]
          }
        },
        {
          "name": "solTreasury",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  45,
                  116,
                  114,
                  101,
                  97,
                  115,
                  117,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "solRewardsVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "solPrizePotVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  106,
                  97,
                  99,
                  107,
                  112,
                  111,
                  116,
                  45,
                  112,
                  111,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "warSolVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "userWarBets",
          "docs": [
            "Per-user, per-cycle bets PDA for the autominer's owner. Created lazily",
            "on the owner's first autominer-driven bet of the cycle (init_if_needed).",
            "Cranker (`caller`) pays rent."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "referrerRewards",
          "docs": [
            "Referrer's commission account. Required when the autominer's owner has a referrer.",
            "SDK derives `[REFERRAL_REWARDS_SEED, player_data.referral_code]`."
          ],
          "writable": true,
          "optional": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  102,
                  101,
                  114,
                  114,
                  97,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "player_data.referral_code",
                "account": "playerData"
              }
            ]
          }
        },
        {
          "name": "caller",
          "docs": [
            "Caller (bot or anyone) - doesn't need to be owner"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "currentRoundId",
          "type": "u64"
        },
        {
          "name": "warId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "expireProgramListing",
      "docs": [
        "Permissionless. Cancels a stale program-owned listing (>= 7 days old)",
        "and re-runs the disposition cascade with progressive expire-discount."
      ],
      "discriminator": [
        177,
        134,
        71,
        155,
        227,
        61,
        2,
        129
      ],
      "accounts": [
        {
          "name": "caller",
          "writable": true,
          "signer": true
        },
        {
          "name": "inventoryPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "inventoryPda",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "inventorySweepVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  115,
                  119,
                  101,
                  101,
                  112,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "rebornEntry",
          "docs": [
            "load/store/close avoids the `Account<T>` Drop-guard re-serialize panic",
            "on the burn paths (where we close the account before exit)."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  98,
                  111,
                  114,
                  110,
                  45,
                  101,
                  110,
                  116,
                  114,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "floorHistory",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  104,
                  105,
                  115,
                  116,
                  111,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "lootboxQueue",
          "docs": [
            "Country lootbox queue for the asset's faction. Seed validated against",
            "the entry's faction_id inside the handler (we deserialize manually)."
          ],
          "writable": true
        },
        {
          "name": "marketplaceConfig",
          "writable": true
        },
        {
          "name": "marketplaceListing",
          "writable": true
        },
        {
          "name": "marketplaceEscrow",
          "writable": true
        },
        {
          "name": "hashbeastAsset",
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "writable": true
        },
        {
          "name": "marketplaceProgram"
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "getGeneBreakdown",
      "docs": [
        "Query function to decode DNA and return gene breakdown",
        "This is a read-only function that can be called via simulateTransaction"
      ],
      "discriminator": [
        22,
        243,
        167,
        62,
        188,
        74,
        20,
        17
      ],
      "accounts": [
        {
          "name": "systemProgram",
          "docs": [
            "System program (required by Anchor but not used)"
          ],
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "dna",
          "type": {
            "array": [
              "u8",
              32
            ]
          }
        }
      ]
    },
    {
      "name": "handleInventoryProceeds",
      "docs": [
        "Permissionless. Splits accrued inventory sale proceeds 50/50 between",
        "sweep vault and sol_treasury."
      ],
      "discriminator": [
        194,
        111,
        19,
        161,
        139,
        123,
        38,
        67
      ],
      "accounts": [
        {
          "name": "caller",
          "signer": true
        },
        {
          "name": "inventoryPool",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "inventoryPda",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "inventorySweepVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  115,
                  119,
                  101,
                  101,
                  112,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "solTreasury",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  45,
                  116,
                  114,
                  101,
                  97,
                  115,
                  117,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "initAutominer",
      "docs": [
        "Initialize autominer vault with flexible faction-direction configuration",
        "use_ticket: Optional ticket tier index. If Some, autominer uses tickets instead of SOL for bets."
      ],
      "discriminator": [
        191,
        245,
        31,
        163,
        225,
        100,
        250,
        21
      ],
      "accounts": [
        {
          "name": "autominerVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "userWallet"
              }
            ]
          }
        },
        {
          "name": "autominerCustody",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "userWallet"
              }
            ]
          }
        },
        {
          "name": "userWallet",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "factionsConfig",
          "type": {
            "option": {
              "defined": {
                "name": "factionsConfig"
              }
            }
          }
        },
        {
          "name": "solPerRound",
          "type": "u64"
        },
        {
          "name": "numRounds",
          "type": "u32"
        },
        {
          "name": "canReload",
          "type": "bool"
        },
        {
          "name": "useTicket",
          "type": {
            "option": "u8"
          }
        }
      ]
    },
    {
      "name": "initHashbeastRoyalties",
      "docs": [
        "Initialize royalties on the HashBeast collection (admin only)"
      ],
      "discriminator": [
        141,
        122,
        39,
        78,
        200,
        192,
        173,
        172
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastsConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "collection",
          "writable": true
        },
        {
          "name": "collectionAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  108,
                  108,
                  101,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "basisPoints",
          "type": "u16"
        },
        {
          "name": "creators",
          "type": {
            "vec": {
              "defined": {
                "name": "creatorInput"
              }
            }
          }
        }
      ]
    },
    {
      "name": "initInventoryPool",
      "docs": [
        "Admin one-shot: initialize inventory pool, floor queue, sale history,",
        "floor history, and the inventory sweep vault. Caches the marketplace",
        "program + config pubkeys for CPI validation on every subsequent ix."
      ],
      "discriminator": [
        243,
        49,
        136,
        91,
        55,
        226,
        103,
        160
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "inventoryPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "floorQueue",
          "docs": [
            "materializing the large fixed array in the generated validator."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  113,
                  117,
                  101,
                  117,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "saleHistory",
          "docs": [
            "materializing the large fixed array in the generated validator."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  97,
                  108,
                  101,
                  45,
                  104,
                  105,
                  115,
                  116,
                  111,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "floorHistory",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  104,
                  105,
                  115,
                  116,
                  111,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "inventorySweepVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  115,
                  119,
                  101,
                  101,
                  112,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "marketplaceProgram",
          "type": "pubkey"
        },
        {
          "name": "marketplaceConfig",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "initLootboxQueue",
      "docs": [
        "Admin one-shot per faction. Creates the country's `LootboxQueue` PDA."
      ],
      "discriminator": [
        138,
        156,
        30,
        170,
        230,
        162,
        186,
        214
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "lootboxQueue",
          "writable": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "factionId",
          "type": "u8"
        }
      ]
    },
    {
      "name": "initialize",
      "docs": [
        "Initialize the global program configuration",
        "This function can only be called once as it creates the program's configuration accounts",
        "It will fail if the accounts already exist"
      ],
      "discriminator": [
        175,
        175,
        109,
        31,
        13,
        152,
        155,
        237
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hodlPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  111,
                  100,
                  108,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "solTreasury",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  45,
                  116,
                  114,
                  101,
                  97,
                  115,
                  117,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "autominerCustody",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "feeRecipient",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "initializeCustodianAccounts",
      "docs": [
        "Initialize both custodian token accounts (admin only)",
        "Initializes:",
        "- MINEBTC custodian: Token-2022 account that holds all staked MINE_BTC tokens (global for all factions)",
        "- Liquidity custodian: Standard SPL Token account that holds all staked LP tokens (global for all factions)"
      ],
      "discriminator": [
        148,
        111,
        73,
        82,
        250,
        213,
        253,
        140
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "degenbtcMint"
        },
        {
          "name": "dbtcCustodian",
          "docs": [
            "degenBTC custodian token account (Token-2022) - PDA owned by dbtc_custodian_authority"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcCustodianAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "lpMint"
        },
        {
          "name": "liquidityCustodian",
          "docs": [
            "Liquidity custodian token account (standard SPL Token) - PDA owned by liquidity_custodian_authority"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  112,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110
                ]
              }
            ]
          }
        },
        {
          "name": "liquidityCustodianAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  112,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "token2022Program",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "rent",
          "address": "SysvarRent111111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "initializeFactionWar",
      "docs": [
        "DONE -::- Initialize a new faction war state PDA.",
        "Must be called once per war cycle before the first round's settle_round.",
        "Permissionless — anyone can initialize the war state for the current war ID."
      ],
      "discriminator": [
        24,
        82,
        166,
        120,
        251,
        50,
        55,
        61
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "warSettlement",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  101,
                  116,
                  116,
                  108,
                  101,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "taxConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  116,
                  97,
                  120,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcMining",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "warId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "initializeGameState",
      "docs": [
        "Initialize the global game state (admin only)",
        "",
        "Sets up the GlobalGameState account that tracks game rounds, betting, and rewards.",
        "This must be called before any rounds can be started."
      ],
      "discriminator": [
        116,
        71,
        118,
        231,
        37,
        192,
        13,
        38
      ],
      "accounts": [
        {
          "name": "globalGameState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "roundDurationSeconds",
          "type": "i64"
        }
      ]
    },
    {
      "name": "initializeHashbeastConfig",
      "docs": [
        "Initialize HashBeastConfig account (admin only).",
        "",
        "Creates the HashBeastConfig that stores collection + breeding state. There",
        "is no lifetime supply cap; only the genesis sale (HashBeastMintConfig) is",
        "bounded."
      ],
      "discriminator": [
        106,
        174,
        71,
        185,
        207,
        51,
        237,
        37
      ],
      "accounts": [
        {
          "name": "hashbeastsConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "initializeHashbeastMintConfig",
      "docs": [
        "Initialize mint-only HashBeast config for the genesis sale."
      ],
      "discriminator": [
        79,
        68,
        33,
        15,
        67,
        3,
        126,
        110
      ],
      "accounts": [
        {
          "name": "hashbeastMintConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "basePrice",
          "type": "u64"
        },
        {
          "name": "curveA",
          "type": "u64"
        },
        {
          "name": "genesisMintLimit",
          "type": "u64"
        },
        {
          "name": "maxGenesisMintsPerFaction",
          "type": "u16"
        }
      ]
    },
    {
      "name": "initializeHashpowerConfig",
      "docs": [
        "Initialize HashpowerConfig account (admin only)"
      ],
      "discriminator": [
        11,
        117,
        248,
        21,
        77,
        88,
        33,
        97
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashpowerConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  112,
                  111,
                  119,
                  101,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "minLockupDays",
          "type": "u64"
        },
        {
          "name": "maxLockupDays",
          "type": "u64"
        },
        {
          "name": "baseMultiplier",
          "type": "u16"
        },
        {
          "name": "maxMultiplier",
          "type": "u16"
        }
      ]
    },
    {
      "name": "initializeMining",
      "docs": [
        "Initialize mining by setting the token vault and emission rate.",
        "Can only be called once. Mining start time is recorded from the on-chain clock."
      ],
      "discriminator": [
        49,
        210,
        169,
        42,
        62,
        114,
        236,
        169
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "vaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "tokenVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "tokenMint",
          "writable": true
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "tokenProgram",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "rent",
          "address": "SysvarRent111111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "dbtcPerRound",
          "type": "u64"
        },
        {
          "name": "poolState",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "initializePlayer",
      "docs": [
        "DONE -::- Initialize a player account for the DegenBTC country arena"
      ],
      "discriminator": [
        79,
        249,
        88,
        177,
        220,
        62,
        56,
        128
      ],
      "accounts": [
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "referrerRewards",
          "docs": [
            "Optional only when no referral code is supplied.",
            "If a referral code is provided, this account must be the canonical referrer's",
            "ReferralRewards PDA and is validated in the instruction handler."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "newPlayerRewards",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  102,
                  101,
                  114,
                  114,
                  97,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "factionId",
          "type": "u8"
        },
        {
          "name": "referralCode",
          "type": {
            "option": "pubkey"
          }
        }
      ]
    },
    {
      "name": "initializeSystemAccounts",
      "docs": [
        "Initialize system referral account and buybacks system (admin only)"
      ],
      "discriminator": [
        82,
        236,
        209,
        56,
        212,
        19,
        210,
        221
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "systemReferralRewards",
          "docs": [
            "Reserved sentinel referral rewards PDA for users who register without a referrer"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  102,
                  101,
                  114,
                  114,
                  97,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "systemProgram"
              }
            ]
          }
        },
        {
          "name": "buybacksAccount",
          "docs": [
            "Buybacks tracking account (admin only)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  98,
                  117,
                  121,
                  98,
                  97,
                  99,
                  107,
                  115
                ]
              }
            ]
          }
        },
        {
          "name": "buybacksSolVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  98,
                  117,
                  121,
                  98,
                  97,
                  99,
                  107,
                  115,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "initializeTaxConfig",
      "docs": [
        "Initialize TaxConfig account and create vault token accounts (admin only).",
        "NFT market making is no longer funded from this tax — it pulls SOL from",
        "`distribute_sol_fees` (see `SolFeeConfig::nft_market_making_pct`)."
      ],
      "discriminator": [
        76,
        114,
        78,
        163,
        170,
        117,
        106,
        161
      ],
      "accounts": [
        {
          "name": "taxConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  116,
                  97,
                  120,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "withdrawWithheldAuthority",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  119,
                  105,
                  116,
                  104,
                  100,
                  114,
                  97,
                  119,
                  45,
                  119,
                  105,
                  116,
                  104,
                  104,
                  101,
                  108,
                  100,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "factionTreasuryVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  116,
                  114,
                  101,
                  97,
                  115,
                  117,
                  114,
                  121,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "degenbtcMint"
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "tokenProgram2022",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "treasuryPct",
          "type": "u8"
        },
        {
          "name": "burnPct",
          "type": "u8"
        }
      ]
    },
    {
      "name": "initializeWarConfig",
      "docs": [
        "Initialize faction_war configuration (admin only).",
        "FactionWar duration is tied to the economy cycle -- one faction_war per LP burn."
      ],
      "discriminator": [
        216,
        240,
        146,
        249,
        181,
        175,
        21,
        105
      ],
      "accounts": [
        {
          "name": "warConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "inventoryFinalizeSale",
      "docs": [
        "Permissionless. Closes a sold inventory `RebornEntry` once the",
        "asset's owner is no longer `inventory_pda` (verified on-chain)."
      ],
      "discriminator": [
        138,
        22,
        237,
        203,
        193,
        218,
        217,
        125
      ],
      "accounts": [
        {
          "name": "caller",
          "writable": true,
          "signer": true
        },
        {
          "name": "inventoryPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "rebornEntry",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  98,
                  111,
                  114,
                  110,
                  45,
                  101,
                  110,
                  116,
                  114,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "reborn_entry.asset",
                "account": "rebornEntry"
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset"
        }
      ],
      "args": []
    },
    {
      "name": "joinBets",
      "docs": [
        "Join a round by placing one or more faction-direction bets."
      ],
      "discriminator": [
        186,
        133,
        82,
        124,
        255,
        74,
        39,
        165
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "docs": [
            "No seeds/bump in derive macro to keep `JoinBets` stack under 4KB."
          ],
          "writable": true
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "gameSession",
          "docs": [
            "GameSession PDA for the current round (must be initialized by crank function)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "arg",
                "path": "roundId"
              }
            ]
          }
        },
        {
          "name": "userGameBet",
          "docs": [
            "UserGameBet PDA for this user's bet in this round"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  98,
                  101,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              },
              {
                "kind": "arg",
                "path": "roundId"
              }
            ]
          }
        },
        {
          "name": "solTreasury",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  45,
                  116,
                  114,
                  101,
                  97,
                  115,
                  117,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "solRewardsVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "solPrizePotVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  106,
                  97,
                  99,
                  107,
                  112,
                  111,
                  116,
                  45,
                  112,
                  111,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "warSolVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "userWarBets",
          "docs": [
            "Per-user, per-cycle bets PDA. Created lazily on the user's first bet",
            "of the cycle (init_if_needed), then read+mutated on subsequent bets and",
            "at round claim time."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  117,
                  115,
                  101,
                  114,
                  45,
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "referrerRewards",
          "docs": [
            "Referrer's commission account. Required when the betting player has a referrer",
            "(`player_data.referral_code != system_program`); the SDK derives the PDA via",
            "`[REFERRAL_REWARDS_SEED, player_data.referral_code]` and passes it here.",
            "Optional only for unreferred players."
          ],
          "writable": true,
          "optional": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  102,
                  101,
                  114,
                  114,
                  97,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "player_data.referral_code",
                "account": "playerData"
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "roundId",
          "type": "u64"
        },
        {
          "name": "warId",
          "type": "u64"
        },
        {
          "name": "betTypes",
          "type": {
            "vec": {
              "defined": {
                "name": "betType"
              }
            }
          }
        },
        {
          "name": "amountPerBet",
          "type": "u64"
        },
        {
          "name": "useTicket",
          "type": {
            "option": "u8"
          }
        }
      ]
    },
    {
      "name": "listUserNft",
      "docs": [
        "User wraps `degenbtc_market::list_nft` and atomically registers the",
        "listing into the floor queue."
      ],
      "discriminator": [
        122,
        254,
        162,
        125,
        244,
        197,
        179,
        86
      ],
      "accounts": [
        {
          "name": "seller",
          "writable": true,
          "signer": true
        },
        {
          "name": "inventoryPool",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "floorQueue",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  113,
                  117,
                  101,
                  117,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "marketplaceConfig",
          "writable": true
        },
        {
          "name": "marketplaceListing",
          "writable": true
        },
        {
          "name": "hashbeastAsset",
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "writable": true
        },
        {
          "name": "marketplaceEscrow",
          "writable": true
        },
        {
          "name": "marketplaceProgram"
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "priceLamports",
          "type": "u64"
        }
      ]
    },
    {
      "name": "rebirthHashbeast",
      "docs": [
        "Rebirth a HashBeast: claim accumulated_val, transfer the NFT into the",
        "global inventory pool for lootbox distribution, or burn it when the",
        "country queue/inventory is full or the rebirth cap is reached."
      ],
      "discriminator": [
        77,
        51,
        186,
        42,
        170,
        213,
        43,
        161
      ],
      "accounts": [
        {
          "name": "user",
          "writable": true,
          "signer": true
        },
        {
          "name": "hashbeastMetadata",
          "docs": [
            "Existing HashBeast metadata account. Mutated in-place (multiplier, xp,",
            "accumulated_val reset to fresh-start values). NOT closed — the same",
            "metadata follows the asset to its next owner."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset",
          "docs": [
            "during the TransferV1 CPI. Currently owned by `user`; becomes owned by",
            "`inventory_pda` after this instruction."
          ],
          "writable": true
        },
        {
          "name": "hashbeastConfig",
          "docs": [
            "Read-only config that pins the canonical HashBeast collection."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "mpl-core helper API, but the handler requires Some and Anchor",
            "address-checks it against HashBeastConfig."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "inventoryPool",
          "docs": [
            "Global inventory pool — counters bumped here. Same PDA acts as the",
            "new owner of the reborn mpl-core asset (custody)."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "inventoryPda",
          "docs": [
            "rewritten to this address by the transfer CPI. It is the *same* PDA",
            "as `inventory_pool` (we just need a separate AccountInfo binding for",
            "mpl-core to see). Validated by seeds."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "rebornEntry",
          "docs": [
            "New per-asset entry created ONLY when the queue had space (asset was",
            "pushed in). When the queue was full, asset is burned and this PDA is",
            "not initialized. Manually init'd inside the handler via",
            "`helper::init_pda_account_if_needed`."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  98,
                  111,
                  114,
                  110,
                  45,
                  101,
                  110,
                  116,
                  114,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "lootboxQueue",
          "docs": [
            "Country lootbox queue for the hashbeast's faction. Pushed into if there's",
            "space; otherwise the asset is burned (no listing fallback)."
          ],
          "writable": true
        },
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcTokenVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "vaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "userTokenAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "user"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "tokenMint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "tokenMint"
        },
        {
          "name": "tokenProgram",
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "recordFloorSnapshot",
      "docs": [
        "Permissionless. Records a daily floor anchor (median of qualifying",
        "user-to-user sales, or queue median fallback)."
      ],
      "discriminator": [
        38,
        78,
        233,
        159,
        238,
        248,
        249,
        196
      ],
      "accounts": [
        {
          "name": "caller",
          "writable": true,
          "signer": true
        },
        {
          "name": "inventoryPool",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "floorQueue",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  113,
                  117,
                  101,
                  117,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "saleHistory",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  97,
                  108,
                  101,
                  45,
                  104,
                  105,
                  115,
                  116,
                  111,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "marketplaceConfig",
          "docs": [
            "InventoryPool so the first thin-volume snapshot can cap itself at the",
            "canonical marketplace minimum price."
          ]
        },
        {
          "name": "queueMedianListing",
          "docs": [
            "Required when FloorQueue has at least one entry."
          ],
          "optional": true
        },
        {
          "name": "queueMedianAsset",
          "optional": true
        },
        {
          "name": "queueMedianEscrow",
          "optional": true
        },
        {
          "name": "floorHistory",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  104,
                  105,
                  115,
                  116,
                  111,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "inventorySweepVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  115,
                  119,
                  101,
                  101,
                  112,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "registerFloorListing",
      "docs": [
        "Permissionless. Registers an existing user listing into the floor queue."
      ],
      "discriminator": [
        6,
        242,
        157,
        146,
        246,
        141,
        195,
        103
      ],
      "accounts": [
        {
          "name": "caller",
          "signer": true
        },
        {
          "name": "inventoryPool",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "floorQueue",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  113,
                  117,
                  101,
                  117,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "marketplaceListing",
          "docs": [
            "inline via owner check + try_deserialize."
          ]
        },
        {
          "name": "marketplaceConfig"
        },
        {
          "name": "hashbeastAsset"
        },
        {
          "name": "marketplaceEscrow",
          "docs": [
            "accepts listings whose asset is still escrow-owned, so stale raw",
            "listing accounts cannot enter the floor queue."
          ]
        },
        {
          "name": "hashbeastMetadata",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        }
      ],
      "args": []
    },
    {
      "name": "requestHashbeastGameplayUnlock",
      "docs": [
        "Request gameplay hashbeast unlock. Actual withdrawal is only available in the next faction_war cycle."
      ],
      "discriminator": [
        116,
        97,
        64,
        58,
        198,
        20,
        201,
        121
      ],
      "accounts": [
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "user",
          "writable": true,
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "setHashbeastFreeMintAllowance",
      "docs": [
        "Set or update a user's free HashBeast mint allowance (admin only).",
        "The user still pays transaction fees and account rent, but not the mint price."
      ],
      "discriminator": [
        67,
        46,
        113,
        132,
        173,
        123,
        100,
        120
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastFreeMintAllowance",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  102,
                  114,
                  101,
                  101,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  97,
                  108,
                  108,
                  111,
                  119,
                  97,
                  110,
                  99,
                  101
                ]
              },
              {
                "kind": "arg",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "user",
          "type": "pubkey"
        },
        {
          "name": "remainingFreeMints",
          "type": "u8"
        }
      ]
    },
    {
      "name": "setPause",
      "docs": [
        "Authority-only kill switch. When paused, the contract blocks new bets",
        "(manual + autominer), new round starts, and hashbeast mints/breeds. Round",
        "settlement, claims, staking, and economy cranks remain available so",
        "players can always exit and pending rounds always finish."
      ],
      "discriminator": [
        63,
        32,
        154,
        2,
        56,
        103,
        79,
        45
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "paused",
          "type": "bool"
        }
      ]
    },
    {
      "name": "setRaydiumPoolState",
      "docs": [
        "Set the Raydium pool state address (admin only)",
        "Security: Prevents using malicious pools for swaps",
        "Also initializes sol_rewards_vault and sol_prize_pot_vault if not already initialized"
      ],
      "discriminator": [
        67,
        39,
        50,
        182,
        86,
        20,
        61,
        67
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "solRewardsVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "solPrizePotVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  106,
                  97,
                  99,
                  107,
                  112,
                  111,
                  116,
                  45,
                  112,
                  111,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "warSolVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "raydiumPoolState",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "settleRound",
      "docs": [
        "DONE -::- Finalize round staking, global jackpot settlement, and faction-war mining."
      ],
      "discriminator": [
        40,
        101,
        18,
        1,
        31,
        129,
        52,
        77
      ],
      "accounts": [
        {
          "name": "globalGameState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "gameSession",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "global_game_state.current_round_id",
                "account": "globalGameSate"
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "factionState",
          "docs": [
            "Winning faction state for updating staker rewards.",
            "Validated manually against the winning faction ID and canonical",
            "`[FACTION_STATE_SEED, supported_factions[winning_faction_id]]` PDA."
          ],
          "writable": true
        },
        {
          "name": "solRewardsVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  116,
                  97,
                  107,
                  101,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "solPrizePotVault",
          "docs": [
            "winning faction has no active stakers, because winner claims are paid",
            "from this vault."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  106,
                  97,
                  99,
                  107,
                  112,
                  111,
                  116,
                  45,
                  112,
                  111,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "docs": [
            "Faction-war config (mut for auto-settle + auto-start)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "arg",
                "path": "warId"
              }
            ]
          }
        },
        {
          "name": "dbtcMining",
          "docs": [
            "Read-only. Lets settle_round check if the LP-burn threshold has",
            "already been crossed and lazily capture `cycle_end_round_id` for",
            "the rare edge case where lp_ops crossed before the cycle's first",
            "round started."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "warId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "settleWar",
      "docs": [
        "DONE -::- Settle faction_war: finalize gameplay-score rankings and compute reward pools.",
        "Permissionless -- anyone can call once the economy cycle's LP burn has completed."
      ],
      "discriminator": [
        165,
        205,
        21,
        245,
        164,
        115,
        160,
        40
      ],
      "accounts": [
        {
          "name": "warConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "war_state.war_id",
                "account": "factionWarState"
              }
            ]
          }
        },
        {
          "name": "warSettlement",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  101,
                  116,
                  116,
                  108,
                  101,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "war_state.war_id",
                "account": "factionWarState"
              }
            ]
          }
        },
        {
          "name": "taxConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  116,
                  97,
                  120,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcMining",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "docs": [
            "Needed to read reward/evolution tuning for `finalize_war_settlement`."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "factionWarSolVault",
          "docs": [
            "sol_treasury at settle. Seeds validated implicitly via the cached bump",
            "on `war_config.rewards_sol_vault_bump` used for the CPI signer."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "solTreasury",
          "docs": [
            "Same PDA that receives protocol-fee SOL from user bets."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  115,
                  111,
                  108,
                  45,
                  116,
                  114,
                  101,
                  97,
                  115,
                  117,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "cranker",
          "docs": [
            "Anyone can settle — no authority check needed."
          ],
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "simulatePurchaseCost",
      "docs": [
        "Simulate mint costs for multiple hashbeasts accounting for bonding curve pricing",
        "",
        "# Parameters",
        "- `hashbeast_config`: HashBeastConfig account",
        "- `hashbeast_mint_config`: HashBeastMintConfig account",
        "- `mint_count`: Number of hashbeasts to mint"
      ],
      "discriminator": [
        1,
        132,
        45,
        54,
        91,
        170,
        150,
        135
      ],
      "accounts": [
        {
          "name": "hashbeastConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastMintConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "mintCount",
          "type": "u64"
        }
      ]
    },
    {
      "name": "snapshotPrice",
      "docs": [
        "INSTRUCTION 1: Take a price snapshot (can be called by anyone every 30 minutes)",
        "Performs a small SOL → MINE_BTC swap for price discovery and earnmarks SOL for POL",
        "After 8 snapshots over 4 hours, call update_rate then add_lp_and_burn to finalize"
      ],
      "discriminator": [
        183,
        215,
        103,
        10,
        224,
        223,
        167,
        69
      ],
      "accounts": [
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "raydiumProgram",
          "docs": [
            "`raydium_cp_swap::ID` so a malicious program can't receive our CPI",
            "(which signs with the `authority_pda` PDA) and drain program-owned",
            "token accounts."
          ],
          "address": "68NJDT912wd5EuCB3jDabR77gCjM3xgfkJ21hUxgfYJ4"
        },
        {
          "name": "poolState",
          "writable": true
        },
        {
          "name": "ammConfig"
        },
        {
          "name": "authorityPda",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "raydiumAuthority"
        },
        {
          "name": "dbtcVault",
          "writable": true
        },
        {
          "name": "solVault",
          "writable": true
        },
        {
          "name": "dbtcTokenAccount",
          "docs": [
            "MINE_BTC token vault (main vault - same as used in initialize_mining)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "solTokenAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "authorityPda"
              },
              {
                "kind": "account",
                "path": "tokenProgram"
              },
              {
                "kind": "account",
                "path": "solMint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "degenbtcMint",
          "writable": true
        },
        {
          "name": "solMint",
          "docs": [
            "— defense-in-depth: Raydium's pool already validates mints against its",
            "vaults, but pinning the address here removes the chance of confusion if",
            "the gating `raydium_pool_state` ever changes."
          ],
          "writable": true,
          "address": "So11111111111111111111111111111111111111112"
        },
        {
          "name": "observationState",
          "writable": true
        },
        {
          "name": "tokenProgram2022",
          "docs": [
            "Token-2022 program for MINE_BTC"
          ],
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        },
        {
          "name": "tokenProgram",
          "docs": [
            "Standard token program for SOL"
          ],
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "buybacksSolVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  98,
                  117,
                  121,
                  98,
                  97,
                  99,
                  107,
                  115,
                  45,
                  115,
                  111,
                  108,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "buybacksAccount",
          "docs": [
            "Buybacks tracking account (required)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  98,
                  117,
                  121,
                  98,
                  97,
                  99,
                  107,
                  115
                ]
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "docs": [
            "System program (required for SOL transfers)"
          ],
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": []
    },
    {
      "name": "stakeDegenbtc",
      "docs": [
        "Stake degenBTC tokens to earn SOL and degenBTC rewards"
      ],
      "discriminator": [
        61,
        185,
        31,
        86,
        49,
        25,
        249,
        62
      ],
      "accounts": [
        {
          "name": "hashpowerConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  112,
                  111,
                  119,
                  101,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "userPosition",
          "writable": true
        },
        {
          "name": "degenbtcMint"
        },
        {
          "name": "userDbtcAccount",
          "docs": [
            "User's degenBTC token account"
          ],
          "writable": true
        },
        {
          "name": "dbtcCustodian",
          "docs": [
            "Token-2022 account that holds staked MINE_BTC for this faction"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "docs": [
            "User who is staking tokens"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "docs": [
            "System program for creating accounts"
          ],
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "tokenProgram",
          "docs": [
            "Token-2022 program for SPL-22 token operations"
          ],
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "lockupDuration",
          "type": "u64"
        },
        {
          "name": "positionIndex",
          "type": "u8"
        }
      ]
    },
    {
      "name": "stakeHashbeast",
      "docs": [
        "Stake a HashBeast to boost hashpower (if faction matches player's faction)"
      ],
      "discriminator": [
        221,
        172,
        128,
        196,
        213,
        157,
        34,
        230
      ],
      "accounts": [
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "taxConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  116,
                  97,
                  120,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastConfig",
          "docs": [
            "Read-only — anchors the `hashbeast_collection` address constraint to",
            "the canonical collection set by admin."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset",
          "docs": [
            "Metaplex Core asset (source of truth for ownership)"
          ],
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "Collection account for the HashBeast. Address-pinned to the official",
            "Core collection recorded in `hashbeast_config.hashbeast_collection`",
            "— without this binding, callers could mint NFT assets outside the",
            "canonical collection, breaking identity / royalties / marketplace",
            "gating. Kept `Option` (rather than required) only to preserve the",
            "existing SDK signature; mint handlers add a runtime `require!(...)`",
            "that the collection is present."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "hashbeastMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeast_metadata.mint",
                "account": "hashBeastMetadata"
              }
            ]
          }
        },
        {
          "name": "hashbeastCustodyPda",
          "docs": [
            "PDA that holds custody of locked NFTs"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "user",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "stakeLpTokens",
      "docs": [
        "Stake LP tokens to earn SOL and degenBTC rewards"
      ],
      "discriminator": [
        142,
        204,
        243,
        84,
        110,
        156,
        243,
        63
      ],
      "accounts": [
        {
          "name": "hashpowerConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  112,
                  111,
                  119,
                  101,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "userPosition",
          "writable": true
        },
        {
          "name": "lpMint"
        },
        {
          "name": "userLpAccount",
          "docs": [
            "User's LP token account"
          ],
          "writable": true
        },
        {
          "name": "liquidityCustodian",
          "docs": [
            "Token account that holds staked LP tokens for this faction"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  112,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "docs": [
            "User who is staking tokens"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "docs": [
            "System program for creating accounts"
          ],
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "tokenProgram",
          "docs": [
            "Token program for SPL token operations"
          ],
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "lockupDuration",
          "type": "u64"
        },
        {
          "name": "positionIndex",
          "type": "u8"
        }
      ]
    },
    {
      "name": "startRound",
      "docs": [
        "DONE -::- Start a new round and initialize its GameSession.",
        "round_id should be current_round_id + 1 (validated in the function)"
      ],
      "discriminator": [
        144,
        144,
        43,
        7,
        193,
        42,
        217,
        215
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalGameState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "gameSession",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  101,
                  115,
                  115,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "arg",
                "path": "roundId"
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warState",
          "docs": [
            "Active cycle's FactionWarState. Seeds are checked against the",
            "current war_id; account must exist (it's created by",
            "`initialize_war_internal`) and be in stage 0 (active).",
            "Enforces that `init_war` ran before any rounds can start — otherwise",
            "`settle_round` would later fail PDA seed validation against a",
            "non-existent war_state, stranding the round in stage 1."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "war_config.current_war_id",
                "account": "factionWarConfig"
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "roundId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "stopAutominer",
      "docs": [
        "Stop autominer and refund remaining SOL"
      ],
      "discriminator": [
        20,
        127,
        226,
        120,
        232,
        185,
        243,
        76
      ],
      "accounts": [
        {
          "name": "autominerVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              }
            ]
          }
        },
        {
          "name": "autominerCustody",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              }
            ]
          }
        },
        {
          "name": "owner",
          "writable": true
        },
        {
          "name": "authority",
          "docs": [
            "Authority (must be owner)"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "sweepFloorLowest",
      "docs": [
        "Permissionless. Buys the cheapest user listing in the floor queue and",
        "disposes (queue / relist / burn) in the same tx. Self-cleans stale",
        "queue entries."
      ],
      "discriminator": [
        50,
        105,
        188,
        192,
        84,
        29,
        43,
        220
      ],
      "accounts": [
        {
          "name": "caller",
          "writable": true,
          "signer": true
        },
        {
          "name": "inventoryPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "inventoryPda",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "inventorySweepVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  115,
                  119,
                  101,
                  101,
                  112,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              }
            ]
          }
        },
        {
          "name": "floorQueue",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  113,
                  117,
                  101,
                  117,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "floorHistory",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  104,
                  105,
                  115,
                  116,
                  111,
                  114,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "rebornEntry",
          "docs": [
            "New entry created on queue/relist paths. PDA seeds enforced; payload",
            "init'd manually inside the handler."
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  98,
                  111,
                  114,
                  110,
                  45,
                  101,
                  110,
                  116,
                  114,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "hashbeastMetadata",
          "docs": [
            "HashBeast metadata (read for faction_id / quality_score)."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "lootboxQueue",
          "docs": [
            "Country lootbox queue for this asset's faction."
          ],
          "writable": true
        },
        {
          "name": "marketplaceConfig",
          "writable": true
        },
        {
          "name": "marketplaceListing",
          "docs": [
            "path C. Address must match floor_queue.entries[0].listing."
          ],
          "writable": true
        },
        {
          "name": "marketplaceEscrow",
          "writable": true
        },
        {
          "name": "hashbeastAsset",
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "writable": true
        },
        {
          "name": "seller",
          "writable": true
        },
        {
          "name": "feeRecipient",
          "writable": true
        },
        {
          "name": "marketplaceProgram"
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "switchHashbeastMining",
      "docs": [
        "Toggle HashBeast NFT minting on/off (admin only)",
        "",
        "Flips is_active between true and false."
      ],
      "discriminator": [
        138,
        10,
        193,
        85,
        55,
        196,
        113,
        35
      ],
      "accounts": [
        {
          "name": "hashbeastMintConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "unstakeDegenbtc",
      "docs": [
        "Unstake degenBTC tokens from a position"
      ],
      "discriminator": [
        169,
        172,
        18,
        108,
        39,
        239,
        20,
        211
      ],
      "accounts": [
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "userPosition",
          "writable": true
        },
        {
          "name": "degenbtcMint",
          "writable": true
        },
        {
          "name": "userDbtcAccount",
          "docs": [
            "User's degenBTC token account to receive the unstaked tokens"
          ],
          "writable": true
        },
        {
          "name": "dbtcCustodian",
          "docs": [
            "Token-2022 account that holds staked MINE_BTC (global for all factions)"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcCustodianAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "docs": [
            "User who is unstaking tokens"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "tokenProgram",
          "docs": [
            "Token-2022 program for SPL-22 token operations"
          ],
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        }
      ],
      "args": [
        {
          "name": "positionIndex",
          "type": "u8"
        }
      ]
    },
    {
      "name": "unstakeHashbeast",
      "docs": [
        "Unstake a HashBeast (remove hashpower boost)"
      ],
      "discriminator": [
        62,
        248,
        56,
        41,
        78,
        49,
        125,
        241
      ],
      "accounts": [
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "taxConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  116,
                  97,
                  120,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastConfig",
          "docs": [
            "Read-only — anchors the `hashbeast_collection` address constraint to",
            "the canonical collection set by admin."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset",
          "docs": [
            "Metaplex Core asset (currently locked in custody PDA)"
          ],
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "Collection account for the HashBeast. Address-pinned to the official",
            "Core collection recorded in `hashbeast_config.hashbeast_collection`",
            "— without this binding, callers could mint NFT assets outside the",
            "canonical collection, breaking identity / royalties / marketplace",
            "gating. Kept `Option` (rather than required) only to preserve the",
            "existing SDK signature; mint handlers add a runtime `require!(...)`",
            "that the collection is present."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "hashbeastMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeast_metadata.mint",
                "account": "hashBeastMetadata"
              }
            ]
          }
        },
        {
          "name": "hashbeastCustodyPda",
          "docs": [
            "PDA that holds custody of locked NFTs"
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "user",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "unstakeLpTokens",
      "docs": [
        "Unstake LP tokens from a position"
      ],
      "discriminator": [
        82,
        157,
        224,
        125,
        196,
        233,
        68,
        199
      ],
      "accounts": [
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "userPosition",
          "writable": true
        },
        {
          "name": "lpMint",
          "writable": true
        },
        {
          "name": "userLpAccount",
          "docs": [
            "User's LP token account to receive the unstaked tokens"
          ],
          "writable": true
        },
        {
          "name": "liquidityCustodian",
          "docs": [
            "Token account that holds staked LP tokens for this faction"
          ],
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  112,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110
                ]
              }
            ]
          }
        },
        {
          "name": "liquidityCustodianAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  112,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  105,
                  97,
                  110,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "docs": [
            "User who is unstaking tokens"
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "tokenProgram",
          "docs": [
            "Token program for SPL token operations"
          ],
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        }
      ],
      "args": [
        {
          "name": "positionIndex",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateAutominer",
      "docs": [
        "Update autominer run controls (add rounds, can_reload)"
      ],
      "discriminator": [
        217,
        167,
        7,
        97,
        251,
        108,
        107,
        52
      ],
      "accounts": [
        {
          "name": "autominerVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              }
            ]
          }
        },
        {
          "name": "autominerCustody",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  117,
                  116,
                  111,
                  109,
                  105,
                  110,
                  101,
                  114,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "autominer_vault.owner",
                "account": "autominerVault"
              }
            ]
          }
        },
        {
          "name": "userWallet",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "roundsAdded",
          "type": {
            "option": "u32"
          }
        },
        {
          "name": "canReload",
          "type": {
            "option": "bool"
          }
        }
      ]
    },
    {
      "name": "updateBreedingConfig",
      "docs": [
        "Update breeding configuration (admin only)"
      ],
      "discriminator": [
        225,
        87,
        232,
        187,
        6,
        108,
        64,
        14
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastsConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "breedingAllowed",
          "type": "bool"
        },
        {
          "name": "breedParentPricesLamports",
          "type": {
            "array": [
              "u64",
              5
            ]
          }
        }
      ]
    },
    {
      "name": "updateCollectionInfo",
      "docs": [
        "Update collection metadata — name and/or URI (admin only)",
        "Useful for fixing dead image URLs or updating collection info"
      ],
      "discriminator": [
        245,
        69,
        45,
        245,
        88,
        189,
        81,
        238
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastsConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "collection",
          "writable": true
        },
        {
          "name": "collectionAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  108,
                  108,
                  101,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "newName",
          "type": {
            "option": "string"
          }
        },
        {
          "name": "newUri",
          "type": {
            "option": "string"
          }
        }
      ]
    },
    {
      "name": "updateConfig",
      "docs": [
        "Propose a new authority (2-step transfer). Only current authority can call.",
        "The proposed authority must call `accept_authority` to complete the transfer."
      ],
      "discriminator": [
        29,
        158,
        252,
        191,
        10,
        83,
        219,
        99
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "newAuthority",
          "type": {
            "option": "pubkey"
          }
        },
        {
          "name": "newFeeRecipient",
          "type": {
            "option": "pubkey"
          }
        }
      ]
    },
    {
      "name": "updateEmissionParams",
      "docs": [
        "Update emission adjustment parameters (admin only)",
        "Allows updating price change threshold and emission increase/decrease percentages"
      ],
      "discriminator": [
        17,
        228,
        142,
        73,
        236,
        237,
        225,
        10
      ],
      "accounts": [
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "priceChangeThreshold",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "emissionIncreasePct",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "emissionDecreasePct",
          "type": {
            "option": "u64"
          }
        }
      ]
    },
    {
      "name": "updateEvolutionUnlockStage",
      "docs": [
        "Update the highest evolution stage unlocked by admin.",
        "`0` disables evolution entirely, `1` allows stage 0 -> 1, etc."
      ],
      "discriminator": [
        127,
        209,
        140,
        159,
        3,
        206,
        118,
        198
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "maxStage",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateFees",
      "docs": [
        "Update fee configuration (admin only)",
        "Validates that percentages sum correctly"
      ],
      "discriminator": [
        225,
        27,
        13,
        6,
        69,
        84,
        172,
        191
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "newProtocolFeePct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "newBuybackPct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "newStakersPct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "newDbtcStakersPct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "newDbtcWinnersPct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "newDbtcSameFactionPct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "newDbtcJackpotPct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "newHodlTaxPct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "snapshotInterval",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "newCycleSolSplitPct",
          "type": {
            "option": "u8"
          }
        },
        {
          "name": "newNftMarketMakingPct",
          "type": {
            "option": "u8"
          }
        }
      ]
    },
    {
      "name": "updateGameState",
      "docs": [
        "Update game state (admin only)",
        "",
        "Optionally pause/resume the game and/or change round duration."
      ],
      "discriminator": [
        96,
        203,
        129,
        158,
        74,
        22,
        229,
        248
      ],
      "accounts": [
        {
          "name": "globalGameState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  103,
                  97,
                  109,
                  101,
                  45,
                  115,
                  116,
                  97,
                  116,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "isActive",
          "type": {
            "option": "bool"
          }
        },
        {
          "name": "roundDurationSeconds",
          "type": {
            "option": "i64"
          }
        }
      ]
    },
    {
      "name": "updateGameplayTuning",
      "docs": [
        "Unified admin surface for gameplay tuning and cycle-reward pacing."
      ],
      "discriminator": [
        92,
        103,
        147,
        8,
        237,
        33,
        203,
        175
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "args",
          "type": {
            "defined": {
              "name": "gameplayTuningUpdateArgs"
            }
          }
        }
      ]
    },
    {
      "name": "updateHashbeastMintConfig",
      "docs": [
        "Update mint-only HashBeast config for genesis sale pricing and caps."
      ],
      "discriminator": [
        195,
        28,
        150,
        79,
        167,
        245,
        236,
        74
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastMintConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "basePrice",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "curveA",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "genesisMintLimit",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "maxGenesisMintsPerFaction",
          "type": {
            "option": "u16"
          }
        }
      ]
    },
    {
      "name": "updateHashpowerConfig",
      "docs": [
        "Update HashpowerConfig account (admin only)"
      ],
      "discriminator": [
        245,
        182,
        174,
        38,
        84,
        86,
        58,
        179
      ],
      "accounts": [
        {
          "name": "hashpowerConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  112,
                  111,
                  119,
                  101,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "minLockupDays",
          "type": "u64"
        },
        {
          "name": "maxLockupDays",
          "type": "u64"
        },
        {
          "name": "baseMultiplier",
          "type": "u16"
        },
        {
          "name": "maxMultiplier",
          "type": "u16"
        }
      ]
    },
    {
      "name": "updateRate",
      "docs": [
        "INSTRUCTION 2a: Update distribution rate (can be called after 4 hours)",
        "Checks if 8 snapshots collected, updates distribution rate, sets flag for LP operation"
      ],
      "discriminator": [
        24,
        225,
        53,
        189,
        72,
        212,
        225,
        178
      ],
      "accounts": [
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        }
      ],
      "args": []
    },
    {
      "name": "updateRpgProgression",
      "docs": [
        "Toggle RPG progression (story events, XP) during gameplay"
      ],
      "discriminator": [
        251,
        78,
        193,
        181,
        237,
        154,
        231,
        31
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "writable": true,
          "signer": true
        }
      ],
      "args": [
        {
          "name": "enabled",
          "type": "bool"
        }
      ]
    },
    {
      "name": "updateTaxConfig",
      "docs": [
        "Update tax distribution percentages (admin only)"
      ],
      "discriminator": [
        61,
        129,
        158,
        221,
        151,
        136,
        36,
        84
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "taxConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  116,
                  97,
                  120,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "signer": true
        }
      ],
      "args": [
        {
          "name": "treasuryPct",
          "type": "u8"
        },
        {
          "name": "burnPct",
          "type": "u8"
        }
      ]
    },
    {
      "name": "updateUserListingPrice",
      "docs": [
        "User updates their listing price; floor queue is re-sorted."
      ],
      "discriminator": [
        243,
        165,
        68,
        3,
        0,
        14,
        91,
        42
      ],
      "accounts": [
        {
          "name": "seller",
          "writable": true,
          "signer": true
        },
        {
          "name": "inventoryPool",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  105,
                  110,
                  118,
                  101,
                  110,
                  116,
                  111,
                  114,
                  121,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "floorQueue",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  108,
                  111,
                  111,
                  114,
                  45,
                  113,
                  117,
                  101,
                  117,
                  101
                ]
              }
            ]
          }
        },
        {
          "name": "marketplaceConfig"
        },
        {
          "name": "marketplaceListing",
          "writable": true
        },
        {
          "name": "hashbeastAsset",
          "docs": [
            "handler reads the listing first and requires listing.asset == this key",
            "before using it as the queue lookup key."
          ]
        },
        {
          "name": "marketplaceProgram"
        }
      ],
      "args": [
        {
          "name": "newPriceLamports",
          "type": "u64"
        }
      ]
    },
    {
      "name": "useHashbeastForGameplay",
      "docs": [
        "Use a HashBeast for gameplay - deposits to custody and sets as active gameplay HashBeast"
      ],
      "discriminator": [
        228,
        29,
        230,
        199,
        165,
        160,
        59,
        82
      ],
      "accounts": [
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "hashbeastConfig",
          "docs": [
            "Read-only — anchors the `hashbeast_collection` address constraint to",
            "the canonical collection set by admin."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset",
          "docs": [
            "Metaplex Core asset"
          ],
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "Collection account for the HashBeast — address-pinned to the official",
            "Core collection. Even with the metadata-PDA invariant guarding all",
            "HashBeasts in circulation, binding here closes the foot-gun of MPL Core",
            "transfer CPIs accepting a wrong-collection account."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "hashbeastMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeast_metadata.mint",
                "account": "hashBeastMetadata"
              }
            ]
          }
        },
        {
          "name": "hashbeastCustodyPda",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "user",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "whitelistMintHashbeast",
      "docs": [
        "Mint a single HashBeast for free using a per-user whitelist allowance.",
        "The caller pays transaction fees and rent, but no HashBeast mint price."
      ],
      "discriminator": [
        173,
        110,
        78,
        131,
        56,
        165,
        136,
        207
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastMintConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "hashbeastFreeMintAllowance",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  102,
                  114,
                  101,
                  101,
                  45,
                  109,
                  105,
                  110,
                  116,
                  45,
                  97,
                  108,
                  108,
                  111,
                  119,
                  97,
                  110,
                  99,
                  101
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset",
          "docs": [
            "Metaplex Core asset (will be created)"
          ],
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "Collection account for the HashBeast. Address-pinned to the official",
            "Core collection recorded in `hashbeast_config.hashbeast_collection`",
            "— without this binding, callers could mint NFT assets outside the",
            "canonical collection, breaking identity / royalties / marketplace",
            "gating. Kept `Option` (rather than required) only to preserve the",
            "existing SDK signature; mint handlers add a runtime `require!(...)`",
            "that the collection is present."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "hashbeastMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeastAsset"
              }
            ]
          }
        },
        {
          "name": "collectionAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  99,
                  111,
                  108,
                  108,
                  101,
                  99,
                  116,
                  105,
                  111,
                  110,
                  95,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "user",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "factionId",
          "type": "u8"
        },
        {
          "name": "ticketTierIndex",
          "type": "u8"
        }
      ]
    },
    {
      "name": "withdrawDbtcRewards",
      "docs": [
        "Withdraw accumulated degenBTC rewards (with HODL tax redistribution)"
      ],
      "discriminator": [
        231,
        134,
        33,
        84,
        132,
        200,
        135,
        59
      ],
      "accounts": [
        {
          "name": "globalConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  103,
                  108,
                  111,
                  98,
                  97,
                  108,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "authority"
              }
            ]
          }
        },
        {
          "name": "referrerRewards",
          "docs": [
            "Optional only when the player has no referrer.",
            "Referred players must provide the canonical referrer's ReferralRewards PDA."
          ],
          "writable": true,
          "optional": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  102,
                  101,
                  114,
                  114,
                  97,
                  108,
                  45,
                  114,
                  101,
                  119,
                  97,
                  114,
                  100,
                  115
                ]
              },
              {
                "kind": "account",
                "path": "player_data.referral_code",
                "account": "playerData"
              }
            ]
          }
        },
        {
          "name": "hodlPool",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  111,
                  100,
                  108,
                  45,
                  112,
                  111,
                  111,
                  108
                ]
              }
            ]
          }
        },
        {
          "name": "degenbtcMint"
        },
        {
          "name": "userDbtcAccount",
          "docs": [
            "User's degenBTC token account to receive rewards"
          ],
          "writable": true
        },
        {
          "name": "dbtcMining",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  105,
                  110,
                  101,
                  45,
                  98,
                  116,
                  99,
                  45,
                  109,
                  105,
                  110,
                  105,
                  110,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "dbtcTokenVault",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  98,
                  116,
                  99,
                  95,
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "dbtcMining"
              }
            ]
          }
        },
        {
          "name": "dbtcVaultAuthority",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  100,
                  101,
                  103,
                  101,
                  110,
                  66,
                  84,
                  67,
                  45,
                  118,
                  97,
                  117,
                  108,
                  116,
                  45,
                  97,
                  117,
                  116,
                  104,
                  111,
                  114,
                  105,
                  116,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "authority",
          "docs": [
            "User claiming rewards"
          ],
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        },
        {
          "name": "tokenProgram",
          "docs": [
            "Token-2022 program for SPL-22 token operations"
          ],
          "address": "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        }
      ],
      "args": []
    },
    {
      "name": "withdrawHashbeastFromGameplay",
      "docs": [
        "Withdraw hashbeast from gameplay - returns hashbeast to user"
      ],
      "discriminator": [
        16,
        194,
        46,
        39,
        231,
        207,
        130,
        87
      ],
      "accounts": [
        {
          "name": "playerData",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  108,
                  97,
                  121,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "user"
              }
            ]
          }
        },
        {
          "name": "factionState",
          "writable": true
        },
        {
          "name": "hashbeastConfig",
          "docs": [
            "Read-only — anchors the `hashbeast_collection` address constraint to",
            "the canonical collection set by admin."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "hashbeastAsset",
          "docs": [
            "Metaplex Core asset (in custody)"
          ],
          "writable": true
        },
        {
          "name": "hashbeastCollection",
          "docs": [
            "Collection account for the HashBeast — address-pinned to the official",
            "Core collection."
          ],
          "writable": true,
          "optional": true
        },
        {
          "name": "hashbeastMetadata",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  109,
                  101,
                  116,
                  97,
                  100,
                  97,
                  116,
                  97
                ]
              },
              {
                "kind": "account",
                "path": "hashbeast_metadata.mint",
                "account": "hashBeastMetadata"
              }
            ]
          }
        },
        {
          "name": "hashbeastCustodyPda",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  104,
                  97,
                  115,
                  104,
                  98,
                  101,
                  97,
                  115,
                  116,
                  45,
                  99,
                  117,
                  115,
                  116,
                  111,
                  100,
                  121
                ]
              }
            ]
          }
        },
        {
          "name": "warConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  102,
                  97,
                  99,
                  116,
                  105,
                  111,
                  110,
                  45,
                  119,
                  97,
                  114,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram",
          "address": "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"
        },
        {
          "name": "user",
          "writable": true,
          "signer": true
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    }
  ],
  "accounts": [
    {
      "name": "autominerVault",
      "discriminator": [
        61,
        88,
        149,
        144,
        247,
        237,
        165,
        91
      ]
    },
    {
      "name": "buybacksAccount",
      "discriminator": [
        116,
        19,
        177,
        176,
        62,
        154,
        106,
        180
      ]
    },
    {
      "name": "degenBtcMining",
      "discriminator": [
        94,
        210,
        75,
        58,
        191,
        108,
        138,
        228
      ]
    },
    {
      "name": "factionState",
      "discriminator": [
        149,
        88,
        216,
        25,
        223,
        222,
        133,
        204
      ]
    },
    {
      "name": "factionWarConfig",
      "discriminator": [
        34,
        254,
        116,
        122,
        150,
        61,
        186,
        149
      ]
    },
    {
      "name": "factionWarSettlement",
      "discriminator": [
        180,
        217,
        17,
        200,
        148,
        211,
        106,
        104
      ]
    },
    {
      "name": "factionWarState",
      "discriminator": [
        213,
        33,
        47,
        109,
        135,
        55,
        185,
        102
      ]
    },
    {
      "name": "floorHistory",
      "discriminator": [
        181,
        105,
        124,
        71,
        145,
        178,
        106,
        110
      ]
    },
    {
      "name": "floorQueue",
      "discriminator": [
        176,
        113,
        230,
        240,
        119,
        207,
        126,
        138
      ]
    },
    {
      "name": "gameSession",
      "discriminator": [
        150,
        116,
        20,
        197,
        205,
        121,
        220,
        240
      ]
    },
    {
      "name": "globalConfig",
      "discriminator": [
        149,
        8,
        156,
        202,
        160,
        252,
        176,
        217
      ]
    },
    {
      "name": "globalGameSate",
      "discriminator": [
        123,
        89,
        107,
        66,
        135,
        161,
        152,
        162
      ]
    },
    {
      "name": "hashBeastConfig",
      "discriminator": [
        236,
        152,
        92,
        4,
        184,
        153,
        85,
        147
      ]
    },
    {
      "name": "hashBeastFreeMintAllowance",
      "discriminator": [
        93,
        155,
        112,
        89,
        139,
        217,
        0,
        112
      ]
    },
    {
      "name": "hashBeastMetadata",
      "discriminator": [
        251,
        57,
        50,
        48,
        137,
        190,
        144,
        201
      ]
    },
    {
      "name": "hashBeastMintConfig",
      "discriminator": [
        58,
        101,
        211,
        209,
        144,
        198,
        182,
        229
      ]
    },
    {
      "name": "hashpowerConfig",
      "discriminator": [
        111,
        230,
        81,
        146,
        156,
        170,
        200,
        27
      ]
    },
    {
      "name": "hodlPool",
      "discriminator": [
        227,
        66,
        200,
        95,
        145,
        192,
        77,
        9
      ]
    },
    {
      "name": "inventoryPool",
      "discriminator": [
        76,
        44,
        179,
        17,
        130,
        28,
        91,
        208
      ]
    },
    {
      "name": "lootboxClaim",
      "discriminator": [
        109,
        38,
        219,
        227,
        152,
        158,
        236,
        129
      ]
    },
    {
      "name": "lootboxQueue",
      "discriminator": [
        170,
        121,
        229,
        160,
        70,
        151,
        108,
        65
      ]
    },
    {
      "name": "playerData",
      "discriminator": [
        197,
        65,
        216,
        202,
        43,
        139,
        147,
        128
      ]
    },
    {
      "name": "rebornEntry",
      "discriminator": [
        239,
        236,
        92,
        143,
        158,
        144,
        112,
        3
      ]
    },
    {
      "name": "referralRewards",
      "discriminator": [
        160,
        0,
        202,
        44,
        122,
        245,
        89,
        169
      ]
    },
    {
      "name": "stakedPosition",
      "discriminator": [
        223,
        6,
        175,
        193,
        36,
        197,
        26,
        4
      ]
    },
    {
      "name": "taxConfig",
      "discriminator": [
        38,
        187,
        35,
        231,
        115,
        102,
        30,
        82
      ]
    },
    {
      "name": "userFactionWarBets",
      "discriminator": [
        211,
        35,
        209,
        130,
        176,
        21,
        97,
        41
      ]
    },
    {
      "name": "userGameBet",
      "discriminator": [
        139,
        132,
        147,
        81,
        217,
        8,
        128,
        248
      ]
    }
  ],
  "events": [
    {
      "name": "autominerInitialized",
      "discriminator": [
        104,
        2,
        58,
        100,
        174,
        132,
        55,
        32
      ]
    },
    {
      "name": "autominerReloaded",
      "discriminator": [
        8,
        244,
        101,
        31,
        150,
        103,
        242,
        65
      ]
    },
    {
      "name": "autominerStopped",
      "discriminator": [
        126,
        253,
        252,
        55,
        61,
        81,
        193,
        240
      ]
    },
    {
      "name": "autominerUpdated",
      "discriminator": [
        112,
        60,
        81,
        170,
        29,
        193,
        110,
        181
      ]
    },
    {
      "name": "betsPlaced",
      "discriminator": [
        33,
        208,
        79,
        174,
        63,
        94,
        22,
        229
      ]
    },
    {
      "name": "collectionDelegateAdded",
      "discriminator": [
        54,
        87,
        48,
        175,
        113,
        235,
        161,
        89
      ]
    },
    {
      "name": "collectionInfoUpdated",
      "discriminator": [
        17,
        159,
        180,
        86,
        167,
        99,
        114,
        150
      ]
    },
    {
      "name": "cycleEndRoundSnapshotted",
      "discriminator": [
        107,
        58,
        109,
        225,
        255,
        150,
        77,
        67
      ]
    },
    {
      "name": "dbtcRewardsClaimed",
      "discriminator": [
        188,
        243,
        2,
        146,
        34,
        167,
        255,
        38
      ]
    },
    {
      "name": "degenBtcStakingRewardsDistributed",
      "discriminator": [
        178,
        147,
        8,
        239,
        127,
        45,
        219,
        28
      ]
    },
    {
      "name": "distributionRateUpdated",
      "discriminator": [
        132,
        142,
        199,
        28,
        219,
        252,
        58,
        203
      ]
    },
    {
      "name": "evolutionUnlockStageUpdated",
      "discriminator": [
        109,
        202,
        109,
        95,
        2,
        148,
        54,
        235
      ]
    },
    {
      "name": "factionAdded",
      "discriminator": [
        181,
        93,
        3,
        66,
        88,
        218,
        14,
        40
      ]
    },
    {
      "name": "factionTreasuryRewardsClaimed",
      "discriminator": [
        43,
        174,
        21,
        93,
        82,
        207,
        14,
        240
      ]
    },
    {
      "name": "factionWarMultiplierUpdated",
      "discriminator": [
        47,
        245,
        254,
        50,
        176,
        47,
        202,
        228
      ]
    },
    {
      "name": "factionWarRewardsClaimed",
      "discriminator": [
        152,
        237,
        151,
        124,
        147,
        141,
        90,
        4
      ]
    },
    {
      "name": "factionWarSettled",
      "discriminator": [
        118,
        174,
        253,
        239,
        54,
        34,
        14,
        17
      ]
    },
    {
      "name": "factionWarStarted",
      "discriminator": [
        38,
        80,
        161,
        135,
        28,
        248,
        170,
        230
      ]
    },
    {
      "name": "floorEntryRegistered",
      "discriminator": [
        216,
        182,
        120,
        12,
        186,
        30,
        4,
        151
      ]
    },
    {
      "name": "floorEntryRemoved",
      "discriminator": [
        121,
        6,
        78,
        42,
        155,
        174,
        182,
        249
      ]
    },
    {
      "name": "floorSnapshotRecorded",
      "discriminator": [
        155,
        25,
        84,
        144,
        183,
        44,
        107,
        62
      ]
    },
    {
      "name": "floorSweepExecuted",
      "discriminator": [
        30,
        136,
        34,
        49,
        28,
        198,
        135,
        228
      ]
    },
    {
      "name": "gamePauseToggled",
      "discriminator": [
        194,
        161,
        165,
        123,
        144,
        162,
        179,
        64
      ]
    },
    {
      "name": "gameplayScoreAccumulated",
      "discriminator": [
        20,
        71,
        235,
        129,
        97,
        129,
        131,
        123
      ]
    },
    {
      "name": "gameplayTuningUpdated",
      "discriminator": [
        207,
        15,
        25,
        2,
        122,
        63,
        72,
        86
      ]
    },
    {
      "name": "hashBeastBred",
      "discriminator": [
        251,
        141,
        238,
        106,
        171,
        15,
        224,
        211
      ]
    },
    {
      "name": "hashBeastCollectionCreated",
      "discriminator": [
        237,
        35,
        3,
        194,
        80,
        172,
        219,
        18
      ]
    },
    {
      "name": "hashBeastEvolution",
      "discriminator": [
        26,
        107,
        123,
        36,
        251,
        10,
        163,
        163
      ]
    },
    {
      "name": "hashBeastFreeMintAllowanceUpdated",
      "discriminator": [
        238,
        87,
        148,
        176,
        130,
        185,
        243,
        167
      ]
    },
    {
      "name": "hashBeastGameplayUnlockRequested",
      "discriminator": [
        95,
        108,
        210,
        152,
        158,
        173,
        161,
        21
      ]
    },
    {
      "name": "hashBeastMinted",
      "discriminator": [
        253,
        88,
        236,
        130,
        110,
        141,
        158,
        201
      ]
    },
    {
      "name": "hashBeastPowerMutation",
      "discriminator": [
        162,
        30,
        193,
        189,
        206,
        248,
        160,
        104
      ]
    },
    {
      "name": "hashBeastRebirthBurned",
      "discriminator": [
        1,
        229,
        100,
        241,
        163,
        71,
        176,
        104
      ]
    },
    {
      "name": "hashBeastReborn",
      "discriminator": [
        106,
        172,
        150,
        42,
        137,
        23,
        235,
        251
      ]
    },
    {
      "name": "hashBeastStaked",
      "discriminator": [
        58,
        169,
        151,
        138,
        178,
        25,
        30,
        13
      ]
    },
    {
      "name": "hashBeastSynced",
      "discriminator": [
        9,
        226,
        41,
        101,
        226,
        243,
        98,
        177
      ]
    },
    {
      "name": "hashBeastUnstaked",
      "discriminator": [
        176,
        12,
        229,
        8,
        218,
        219,
        186,
        220
      ]
    },
    {
      "name": "hashBeastUsedForGameplay",
      "discriminator": [
        90,
        77,
        52,
        33,
        29,
        64,
        181,
        153
      ]
    },
    {
      "name": "hashBeastVisualMutation",
      "discriminator": [
        93,
        158,
        186,
        28,
        234,
        18,
        232,
        135
      ]
    },
    {
      "name": "hashBeastWithdrawnFromGameplay",
      "discriminator": [
        142,
        250,
        134,
        114,
        160,
        237,
        42,
        180
      ]
    },
    {
      "name": "hodlTaxRedistributed",
      "discriminator": [
        78,
        108,
        26,
        3,
        253,
        50,
        154,
        151
      ]
    },
    {
      "name": "inventoryAssetBurned",
      "discriminator": [
        71,
        52,
        63,
        108,
        142,
        204,
        55,
        225
      ]
    },
    {
      "name": "inventoryAssetRelisted",
      "discriminator": [
        4,
        118,
        211,
        33,
        197,
        223,
        195,
        132
      ]
    },
    {
      "name": "inventoryPoolInitialized",
      "discriminator": [
        27,
        13,
        54,
        5,
        135,
        60,
        9,
        215
      ]
    },
    {
      "name": "inventoryProceedsRouted",
      "discriminator": [
        167,
        195,
        1,
        187,
        231,
        84,
        170,
        23
      ]
    },
    {
      "name": "inventorySaleFinalized",
      "discriminator": [
        89,
        86,
        229,
        27,
        177,
        37,
        195,
        24
      ]
    },
    {
      "name": "jackpotHit",
      "discriminator": [
        82,
        164,
        217,
        72,
        32,
        247,
        232,
        163
      ]
    },
    {
      "name": "liquidityAdded",
      "discriminator": [
        154,
        26,
        221,
        108,
        238,
        64,
        217,
        161
      ]
    },
    {
      "name": "liquidityStaked",
      "discriminator": [
        146,
        198,
        60,
        153,
        104,
        52,
        241,
        249
      ]
    },
    {
      "name": "liquidityUnstaked",
      "discriminator": [
        99,
        96,
        110,
        82,
        51,
        48,
        143,
        72
      ]
    },
    {
      "name": "lootboxNftClaimed",
      "discriminator": [
        44,
        242,
        11,
        139,
        114,
        98,
        254,
        188
      ]
    },
    {
      "name": "lootboxQueueInitialized",
      "discriminator": [
        249,
        30,
        144,
        36,
        33,
        92,
        48,
        135
      ]
    },
    {
      "name": "lootboxQueuePush",
      "discriminator": [
        160,
        89,
        195,
        196,
        185,
        204,
        190,
        56
      ]
    },
    {
      "name": "lootboxRollMissed",
      "discriminator": [
        70,
        239,
        245,
        48,
        43,
        79,
        102,
        34
      ]
    },
    {
      "name": "lootboxRollWon",
      "discriminator": [
        222,
        119,
        14,
        137,
        243,
        239,
        32,
        149
      ]
    },
    {
      "name": "lpStakingRewardsDistributed",
      "discriminator": [
        24,
        78,
        145,
        239,
        225,
        166,
        65,
        6
      ]
    },
    {
      "name": "lpTokensBurned",
      "discriminator": [
        204,
        43,
        106,
        124,
        185,
        153,
        93,
        47
      ]
    },
    {
      "name": "mineBtcStaked",
      "discriminator": [
        201,
        216,
        210,
        187,
        67,
        45,
        132,
        10
      ]
    },
    {
      "name": "mineBtcUnstaked",
      "discriminator": [
        56,
        98,
        172,
        52,
        192,
        42,
        136,
        198
      ]
    },
    {
      "name": "minebtcClaimableAccrued",
      "discriminator": [
        18,
        23,
        247,
        3,
        40,
        250,
        208,
        69
      ]
    },
    {
      "name": "miningTokenVaultSet",
      "discriminator": [
        107,
        208,
        54,
        223,
        77,
        26,
        58,
        165
      ]
    },
    {
      "name": "nftMarketMakingFunded",
      "discriminator": [
        234,
        16,
        154,
        135,
        49,
        56,
        120,
        158
      ]
    },
    {
      "name": "paperHandBurned",
      "discriminator": [
        177,
        69,
        66,
        129,
        91,
        9,
        135,
        253
      ]
    },
    {
      "name": "playerInitialized",
      "discriminator": [
        214,
        37,
        153,
        142,
        63,
        109,
        206,
        15
      ]
    },
    {
      "name": "playerRecruited",
      "discriminator": [
        176,
        105,
        56,
        74,
        119,
        8,
        77,
        6
      ]
    },
    {
      "name": "priceSnapshotTaken",
      "discriminator": [
        56,
        79,
        185,
        223,
        51,
        132,
        9,
        2
      ]
    },
    {
      "name": "programListingExpired",
      "discriminator": [
        179,
        165,
        15,
        52,
        128,
        19,
        147,
        244
      ]
    },
    {
      "name": "referralRewardsClaimed",
      "discriminator": [
        178,
        107,
        76,
        169,
        252,
        154,
        45,
        235
      ]
    },
    {
      "name": "rewardsDistributedForRound",
      "discriminator": [
        163,
        179,
        202,
        95,
        41,
        107,
        119,
        213
      ]
    },
    {
      "name": "roundEnded",
      "discriminator": [
        70,
        113,
        6,
        162,
        176,
        78,
        201,
        19
      ]
    },
    {
      "name": "roundRewardsClaimed",
      "discriminator": [
        204,
        36,
        30,
        87,
        163,
        46,
        38,
        91
      ]
    },
    {
      "name": "roundStarted",
      "discriminator": [
        180,
        209,
        2,
        244,
        238,
        48,
        170,
        120
      ]
    },
    {
      "name": "solFeesWithdrawn",
      "discriminator": [
        189,
        119,
        134,
        90,
        202,
        241,
        174,
        251
      ]
    },
    {
      "name": "solRewardsClaimed",
      "discriminator": [
        95,
        66,
        67,
        66,
        92,
        38,
        237,
        198
      ]
    },
    {
      "name": "storyEventTriggered",
      "discriminator": [
        24,
        161,
        142,
        60,
        38,
        21,
        143,
        205
      ]
    },
    {
      "name": "taxDistributed",
      "discriminator": [
        0,
        184,
        161,
        141,
        246,
        77,
        185,
        43
      ]
    },
    {
      "name": "userSaleRecorded",
      "discriminator": [
        125,
        2,
        110,
        144,
        236,
        23,
        27,
        181
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "invalidAuthority",
      "msg": "The authority is invalid"
    },
    {
      "code": 6001,
      "name": "invalidReferralAccount",
      "msg": "Invalid referral account"
    },
    {
      "code": 6002,
      "name": "unauthorized",
      "msg": "User not authorized to perform this action"
    },
    {
      "code": 6003,
      "name": "noPendingAuthority",
      "msg": "No pending authority transfer to accept"
    },
    {
      "code": 6004,
      "name": "invalidMint",
      "msg": "Invalid mint"
    },
    {
      "code": 6005,
      "name": "arithmeticOverflow",
      "msg": "Arithmetic overflow"
    },
    {
      "code": 6006,
      "name": "miningAlreadyInitialized",
      "msg": "Mining already initialized"
    },
    {
      "code": 6007,
      "name": "referralCannotBeSameAsOwner",
      "msg": "Referral pubkey cannot be the same as owner"
    },
    {
      "code": 6008,
      "name": "referralRewardsAccountRequired",
      "msg": "Referral rewards account required when referral code is provided"
    },
    {
      "code": 6009,
      "name": "invalidParameters",
      "msg": "Invalid parameters provided for operation"
    },
    {
      "code": 6010,
      "name": "gamePaused",
      "msg": "Game is paused — new bets, autominer execution, round starts, and hashbeast mints are disabled. Claims and round settlement still work."
    },
    {
      "code": 6011,
      "name": "amountOverflow",
      "msg": "Amount overflow"
    },
    {
      "code": 6012,
      "name": "miningNotInitialized",
      "msg": "Mining not initialized yet"
    },
    {
      "code": 6013,
      "name": "tokenVaultNotInitialized",
      "msg": "Token vault not initialized"
    },
    {
      "code": 6014,
      "name": "insufficientTokensInVault",
      "msg": "Insufficient tokens in vault"
    },
    {
      "code": 6015,
      "name": "invalidFactionName",
      "msg": "Invalid faction name: must be 1-16 characters"
    },
    {
      "code": 6016,
      "name": "maxFactionsReached",
      "msg": "Maximum number of factions reached (12 max)"
    },
    {
      "code": 6017,
      "name": "factionAlreadyExists",
      "msg": "Faction with this name already exists"
    },
    {
      "code": 6018,
      "name": "invalidFactionId",
      "msg": "Invalid faction ID: faction does not exist"
    },
    {
      "code": 6019,
      "name": "invalidMplCoreProgram",
      "msg": "Metaplex Core program ID mismatch"
    },
    {
      "code": 6020,
      "name": "invalidAccount",
      "msg": "Invalid account provided"
    },
    {
      "code": 6021,
      "name": "invalidMetadata",
      "msg": "Invalid metadata provided"
    },
    {
      "code": 6022,
      "name": "uriTooLong",
      "msg": "URI too long - maximum 200 characters"
    },
    {
      "code": 6023,
      "name": "hashBeastAlreadyAtGuard",
      "msg": "HashBeast already at guard"
    },
    {
      "code": 6024,
      "name": "hashBeastNotAtGuard",
      "msg": "HashBeast is not incubated in this degenBTC program"
    },
    {
      "code": 6025,
      "name": "hashBeastLimitExceeded",
      "msg": "HashBeast limit for this tier has been reached"
    },
    {
      "code": 6026,
      "name": "nftNotOwnedByUser",
      "msg": "NFT is not owned by the user"
    },
    {
      "code": 6027,
      "name": "updateDistRateFirst",
      "msg": "Update dist rate first"
    },
    {
      "code": 6028,
      "name": "maxLimitError",
      "msg": "degenBtc needed for POl cannot be more than 5% of vault balance"
    },
    {
      "code": 6029,
      "name": "roundEnded",
      "msg": "Round has already ended"
    },
    {
      "code": 6030,
      "name": "roundNotEnded",
      "msg": "Round has not ended yet"
    },
    {
      "code": 6031,
      "name": "invalidRound",
      "msg": "Invalid round ID"
    },
    {
      "code": 6032,
      "name": "noFactions",
      "msg": "No factions provided"
    },
    {
      "code": 6033,
      "name": "noBets",
      "msg": "No bets placed in this round"
    },
    {
      "code": 6034,
      "name": "factionNotFound",
      "msg": "Faction not found"
    },
    {
      "code": 6035,
      "name": "noRoundsRemaining",
      "msg": "No rounds remaining in autominer"
    },
    {
      "code": 6036,
      "name": "invalidOwner",
      "msg": "Invalid owner"
    },
    {
      "code": 6037,
      "name": "invalidAmount",
      "msg": "Invalid amount"
    },
    {
      "code": 6038,
      "name": "betBelowMinimum",
      "msg": "Minimum SOL bet per country-direction position is 0.0001 SOL"
    },
    {
      "code": 6039,
      "name": "insufficientFunds",
      "msg": "Insufficient funds"
    },
    {
      "code": 6040,
      "name": "invalidInitType",
      "msg": "Invalid init type"
    },
    {
      "code": 6041,
      "name": "invalidState",
      "msg": "Invalid state for this operation"
    },
    {
      "code": 6042,
      "name": "invalidProgramId",
      "msg": "Invalid program ID"
    },
    {
      "code": 6043,
      "name": "noCreators",
      "msg": "No creators specified"
    },
    {
      "code": 6044,
      "name": "invalidCreatorShare",
      "msg": "Sum of creator percentages must be 100"
    },
    {
      "code": 6045,
      "name": "royaltiesPluginMissing",
      "msg": "Royalties plugin not found on collection"
    },
    {
      "code": 6046,
      "name": "unexpectedRuleSetVariant",
      "msg": "Royalties rule_set is not ProgramDenyList"
    },
    {
      "code": 6047,
      "name": "invalidStage",
      "msg": "Invalid stage"
    },
    {
      "code": 6048,
      "name": "cannotBeginRound",
      "msg": "Cannot begin round"
    },
    {
      "code": 6049,
      "name": "positionNotLocked",
      "msg": "Position not unlocked"
    },
    {
      "code": 6050,
      "name": "breedingNotAllowed",
      "msg": "Breeding is not currently allowed"
    },
    {
      "code": 6051,
      "name": "maxBreedCountReached",
      "msg": "Maximum breed count reached for this hashbeast"
    },
    {
      "code": 6052,
      "name": "cooldownNotEnded",
      "msg": "Breeding cooldown has not ended yet"
    },
    {
      "code": 6053,
      "name": "maxEvolutionReached",
      "msg": "Maximum evolution stage reached"
    },
    {
      "code": 6054,
      "name": "maxRebirthCountReached",
      "msg": "Maximum rebirth count reached for this hashbeast"
    },
    {
      "code": 6055,
      "name": "hashBeastMetadataNotFound",
      "msg": "HashBeast metadata not found"
    },
    {
      "code": 6056,
      "name": "rebirthLevelMismatch",
      "msg": "HashBeasts must be at the same rebirth generation to breed"
    },
    {
      "code": 6057,
      "name": "invalidBreedingPair",
      "msg": "Invalid breeding pair"
    },
    {
      "code": 6058,
      "name": "breedFloorAnchorUnavailable",
      "msg": "Breeding floor anchor is unavailable or too low"
    },
    {
      "code": 6059,
      "name": "dbtcPriceUnavailable",
      "msg": "dbTC price is unavailable for breeding"
    },
    {
      "code": 6060,
      "name": "genesisNotSoldOut",
      "msg": "Genesis HashBeast mint sale must be sold out before breeding"
    },
    {
      "code": 6061,
      "name": "positionAlreadyExists",
      "msg": "Position already exists"
    },
    {
      "code": 6062,
      "name": "claimPendingRoundRewards",
      "msg": "HashBeast DNA mismatch"
    },
    {
      "code": 6063,
      "name": "mintingNotAllowed",
      "msg": "Minting not allowed"
    },
    {
      "code": 6064,
      "name": "gameplayNotEnabled",
      "msg": "Gameplay locking is only available while RPG progression is enabled"
    },
    {
      "code": 6065,
      "name": "gameplayUnlockAlreadyRequested",
      "msg": "Gameplay unlock has already been requested for this hashbeast"
    },
    {
      "code": 6066,
      "name": "gameplayUnlockNotRequested",
      "msg": "Gameplay unlock has not been requested"
    },
    {
      "code": 6067,
      "name": "gameplayUnlockNotReady",
      "msg": "Gameplay hashbeast can only be unlocked after the next faction_war cycle begins"
    },
    {
      "code": 6068,
      "name": "gameplayRewardsPending",
      "msg": "Claim all pending round and faction-war reward accounts before unlocking this gameplay hashbeast"
    },
    {
      "code": 6069,
      "name": "factionWarNotActive",
      "msg": "FactionWar is not currently active"
    },
    {
      "code": 6070,
      "name": "factionWarNotEnded",
      "msg": "FactionWar has not ended yet"
    },
    {
      "code": 6071,
      "name": "factionWarNotSettled",
      "msg": "FactionWar has not been settled yet"
    },
    {
      "code": 6072,
      "name": "factionWarAlreadySettled",
      "msg": "FactionWar has already been settled"
    },
    {
      "code": 6073,
      "name": "factionWarRewardsAlreadyClaimed",
      "msg": "FactionWar rewards have already been claimed"
    },
    {
      "code": 6074,
      "name": "roundFinalizationPending",
      "msg": "Round is pending faction-reward finalization; settle cannot run between end_round and settle_round"
    },
    {
      "code": 6075,
      "name": "cycleAwaitingSettlement",
      "msg": "Cycle has reached its final round; war must be settled before a new round can start"
    },
    {
      "code": 6076,
      "name": "ticketBetCapExceeded",
      "msg": "Ticket-backed bets exceed the session cap"
    },
    {
      "code": 6077,
      "name": "roundEntropyNotReady",
      "msg": "Round entropy is not ready yet"
    },
    {
      "code": 6078,
      "name": "maxFreeHashBeastMintsExceeded",
      "msg": "Free HashBeast mint allowance exceeds the per-user maximum"
    },
    {
      "code": 6079,
      "name": "noFreeHashBeastMintsRemaining",
      "msg": "No free HashBeast mints remaining for this user"
    },
    {
      "code": 6080,
      "name": "inventoryFull",
      "msg": "Inventory pool is at MAX_INVENTORY capacity"
    },
    {
      "code": 6081,
      "name": "invalidRebornStatus",
      "msg": "Reborn entry is in an invalid status for this operation"
    },
    {
      "code": 6082,
      "name": "invalidMarketplaceProgram",
      "msg": "Marketplace program account does not match cached pubkey"
    },
    {
      "code": 6083,
      "name": "invalidMarketplaceConfig",
      "msg": "Marketplace config account does not match cached pubkey"
    },
    {
      "code": 6084,
      "name": "listingPriceTooLow",
      "msg": "Inventory listing price is below the marketplace minimum"
    },
    {
      "code": 6085,
      "name": "listingPriceExceedsMax",
      "msg": "Listing price exceeds the buyer's max price"
    },
    {
      "code": 6086,
      "name": "assetNotInInventory",
      "msg": "Inventory PDA does not own this asset"
    },
    {
      "code": 6087,
      "name": "floorQueueEmpty",
      "msg": "Floor queue has no entries"
    },
    {
      "code": 6088,
      "name": "floorQueueFull",
      "msg": "Floor queue is full and the new entry is not cheaper than the worst entry"
    },
    {
      "code": 6089,
      "name": "assetAlreadyInQueue",
      "msg": "Asset is already registered in the floor queue"
    },
    {
      "code": 6090,
      "name": "noLiveFloorEntries",
      "msg": "Cached floor entry is stale and was popped; nothing to sweep this tx"
    },
    {
      "code": 6091,
      "name": "floorPriceTooHigh",
      "msg": "Listing price exceeds the attractive ceiling vs current anchor"
    },
    {
      "code": 6092,
      "name": "sweepVaultBelowReserve",
      "msg": "Sweep would drop the vault below MIN_SWEEP_RESERVE_LAMPORTS"
    },
    {
      "code": 6093,
      "name": "sweepTxCapExceeded",
      "msg": "Sweep tx exceeds the per-tx sweep cap"
    },
    {
      "code": 6094,
      "name": "sweepAnchorTooLow",
      "msg": "Sweep anchor is below the minimum sweep threshold (no recent volume)"
    },
    {
      "code": 6095,
      "name": "floorAnchorStale",
      "msg": "Floor anchor is stale; record a fresh floor snapshot before using floor support"
    },
    {
      "code": 6096,
      "name": "staleFloorEntry",
      "msg": "Floor entry data does not match the live marketplace listing"
    },
    {
      "code": 6097,
      "name": "programListingNotAllowed",
      "msg": "Program-owned listings cannot be registered in the floor queue"
    },
    {
      "code": 6098,
      "name": "listingNotYetExpirable",
      "msg": "Listing has not yet aged enough for expire_program_listing"
    },
    {
      "code": 6099,
      "name": "notProgramListing",
      "msg": "This ix only operates on program-owned listings"
    },
    {
      "code": 6100,
      "name": "notUserListing",
      "msg": "This ix only operates on user-owned listings"
    },
    {
      "code": 6101,
      "name": "snapshotTooSoon",
      "msg": "Floor snapshot was already recorded within the cadence window"
    },
    {
      "code": 6102,
      "name": "snapshotSwapOutputTooLow",
      "msg": "Oracle snapshot swap output is below the minimum acceptable amount"
    },
    {
      "code": 6103,
      "name": "snapshotPriceDeviationTooHigh",
      "msg": "Oracle snapshot price deviates too far from the recent weighted price"
    },
    {
      "code": 6104,
      "name": "assetStillOwnedByInventory",
      "msg": "Asset is still owned by inventory_pda — sale not actually settled"
    },
    {
      "code": 6105,
      "name": "assetStillListed",
      "msg": "Asset is still held by marketplace escrow — listing has not sold"
    },
    {
      "code": 6106,
      "name": "listingNotInQueue",
      "msg": "Listing is not present in the floor queue"
    }
  ],
  "types": [
    {
      "name": "autominerFactionPick",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "direction",
            "type": {
              "defined": {
                "name": "predictionDirection"
              }
            }
          }
        ]
      }
    },
    {
      "name": "autominerInitialized",
      "docs": [
        "Event emitted when an autominer vault is initialized"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "gameplayHashbeast",
            "type": "pubkey"
          },
          {
            "name": "autominerVault",
            "type": "pubkey"
          },
          {
            "name": "solPerRound",
            "type": "u64"
          },
          {
            "name": "numRounds",
            "type": "u32"
          },
          {
            "name": "betsPerRound",
            "type": "u64"
          },
          {
            "name": "betSizePerBet",
            "type": "u64"
          },
          {
            "name": "hasFactionsConfig",
            "type": "bool"
          },
          {
            "name": "canReload",
            "type": "bool"
          },
          {
            "name": "useTicket",
            "type": {
              "option": "u8"
            }
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "autominerReloaded",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "autominerVault",
            "type": "pubkey"
          },
          {
            "name": "roundsToAdd",
            "type": "u32"
          },
          {
            "name": "solForRounds",
            "type": "u64"
          },
          {
            "name": "leftoverSol",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "autominerStopped",
      "docs": [
        "Event emitted when autominer is stopped"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "autominerVault",
            "type": "pubkey"
          },
          {
            "name": "roundsRemaining",
            "type": "u32"
          },
          {
            "name": "refundAmount",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "autominerUpdated",
      "docs": [
        "Event emitted when autominer is initialized"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "autominerVault",
            "type": "pubkey"
          },
          {
            "name": "solPerRound",
            "type": "u64"
          },
          {
            "name": "roundsRemaining",
            "type": "u32"
          },
          {
            "name": "canReload",
            "type": "bool"
          },
          {
            "name": "solDiff",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "autominerVault",
      "docs": [
        "Autominer Vault PDA (Seed: `[b\"autominer\", user_pubkey]`)",
        "Stores autominer configuration for a user; funds are held in the global autominer custody PDA",
        "Allows users to configure automatic faction-direction betting."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "factionsConfig",
            "docs": [
              "Factions configuration (specific list or random count with direction) - optional"
            ],
            "type": {
              "option": {
                "defined": {
                  "name": "factionsConfig"
                }
              }
            }
          },
          {
            "name": "solPerRound",
            "docs": [
              "SOL reserved per round.",
              "- SOL mode: total round budget, including keeper compensation plus generated bets.",
              "- Ticket mode: must be 0; a fixed keeper reserve is deposited per round."
            ],
            "type": "u64"
          },
          {
            "name": "roundsRemaining",
            "docs": [
              "Number of rounds remaining (decremented after each round)"
            ],
            "type": "u32"
          },
          {
            "name": "lastBetRoundId",
            "docs": [
              "Last round ID where bets were placed (to prevent duplicate bets)"
            ],
            "type": "u64"
          },
          {
            "name": "vaultBump",
            "type": "u8"
          },
          {
            "name": "solBalance",
            "docs": [
              "Remaining SOL balance reserved for this autominer (held in autominer custody PDA)"
            ],
            "type": "u64"
          },
          {
            "name": "canReload",
            "docs": [
              "If set to true, SOL rewards can be used to reload Autominer and continue mining degenBTC"
            ],
            "type": "bool"
          },
          {
            "name": "useTicket",
            "docs": [
              "Optional ticket tier index. If Some, autominer uses tickets for bet points.",
              "Ticket mode still reserves SOL upfront to compensate the keeper for each execution.",
              "Bet amount is determined by the ticket value in player_data.free_tickets[tier]."
            ],
            "type": {
              "option": "u8"
            }
          },
          {
            "name": "pendingAutominerClaims",
            "docs": [
              "Autominer-placed bets that haven't been claimed yet. Incremented in",
              "`execute_autominer_bet` after a successful bet, decremented in",
              "`claim_autominer_rewards`. When this hits 0 in claim, that claim is the",
              "last unclaimed bet of the current funded cycle → bulk-reload trigger."
            ],
            "type": "u32"
          },
          {
            "name": "accruedReloadSol",
            "docs": [
              "SOL rewards won during the current funded cycle, parked in",
              "`autominer_custody`, awaiting bulk-conversion to additional rounds on",
              "the final claim. Refunded to owner on `stop_autominer`."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "betType",
      "docs": [
        "Bet type enum for user bets.",
        "Each bet selects a faction and a direction for the active faction_war."
      ],
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "factionDirection",
            "fields": [
              {
                "name": "factionId",
                "type": "u8"
              },
              {
                "name": "direction",
                "type": {
                  "defined": {
                    "name": "predictionDirection"
                  }
                }
              }
            ]
          }
        ]
      }
    },
    {
      "name": "betsPlaced",
      "docs": [
        "Event emitted when bets are placed (single, batch, or autominer)"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "gameplayHashbeast",
            "type": "pubkey"
          },
          {
            "name": "gameplayHashbeastDna",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "activeMultiplier",
            "type": "u32"
          },
          {
            "name": "gameplayHashbeastXp",
            "type": "u32"
          },
          {
            "name": "roundId",
            "type": "u64"
          },
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "numBets",
            "type": "u8"
          },
          {
            "name": "factionIds",
            "type": "bytes"
          },
          {
            "name": "directions",
            "type": "bytes"
          },
          {
            "name": "netAmounts",
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "feeAmounts",
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "pointsAmounts",
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "wgtdPointsAmounts",
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "usedTicket",
            "type": "bool"
          },
          {
            "name": "ticketTypeIndex",
            "type": {
              "option": "u8"
            }
          },
          {
            "name": "isAutominer",
            "type": "bool"
          },
          {
            "name": "autominerVault",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "caller",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "callerCompensation",
            "type": "u64"
          },
          {
            "name": "roundsRemaining",
            "type": {
              "option": "u32"
            }
          },
          {
            "name": "vaultClosed",
            "type": {
              "option": "bool"
            }
          },
          {
            "name": "totalCycleSolSplit",
            "docs": [
              "Total SOL deducted from this batch for the cycle SOL split (faction war vault)."
            ],
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "buybacksAccount",
      "docs": [
        "Buybacks account that accumulates SOL for token buybacks"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "totalSolAccumulated",
            "docs": [
              "Total SOL accumulated for buybacks (in lamports)"
            ],
            "type": "u64"
          },
          {
            "name": "solForPol",
            "docs": [
              "SOL earmarked for Protocol Owned Liquidity (in lamports)"
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "collectionDelegateAdded",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "collection",
            "type": "pubkey"
          },
          {
            "name": "delegate",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "collectionInfoUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "collection",
            "type": "pubkey"
          },
          {
            "name": "newName",
            "type": {
              "option": "string"
            }
          },
          {
            "name": "newUri",
            "type": {
              "option": "string"
            }
          }
        ]
      }
    },
    {
      "name": "creatorInput",
      "docs": [
        "Helper type for passing creators from client"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "address",
            "type": "pubkey"
          },
          {
            "name": "percentage",
            "docs": [
              "Whole-percent share (`100` = 100%). Sum must equal the percentage denominator."
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "cycleEndRoundSnapshotted",
      "docs": [
        "Emitted by `add_lp_and_burn` when an LP operation pushes",
        "`lp_operations_count` past `settle_at_lp_op_count` and the current cycle's",
        "final round_id is snapshotted onto `FactionWarConfig.cycle_end_round_id`.",
        "Lets indexers know the active cycle is now in its final round — no new",
        "rounds will start until settle_war runs."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "cycleEndRoundId",
            "type": "u64"
          },
          {
            "name": "lpOperationsCount",
            "type": "u32"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "dbtcRewardsClaimed",
      "docs": [
        "Event emitted when a user withdraws gameplay-earned degenBTC token rewards."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "dbtcAmount",
            "type": "u64"
          },
          {
            "name": "hodlTax",
            "type": "u64"
          },
          {
            "name": "referralBonus",
            "type": "u64"
          },
          {
            "name": "referralReward",
            "type": "u64"
          },
          {
            "name": "referrer",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "degenBtcDistConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "dbtcStakersPct",
            "docs": [
              "Whole-percent share of degenBTC emission that goes to stakers. `100` = 100%."
            ],
            "type": "u8"
          },
          {
            "name": "dbtcWinnersPct",
            "docs": [
              "Whole-percent share of degenBTC emission that goes to winning faction bettors. `100` = 100%."
            ],
            "type": "u8"
          },
          {
            "name": "dbtcSameFactionPct",
            "docs": [
              "Whole-percent share of degenBTC emission that goes to each non-winning",
              "direction on the winning faction. With 3 total directions, up to two",
              "losing directions may each receive this share if they have bettors."
            ],
            "type": "u8"
          },
          {
            "name": "dbtcJackpotPct",
            "docs": [
              "Whole-percent share of degenBTC emission that goes to the global jackpot. `100` = 100%."
            ],
            "type": "u8"
          },
          {
            "name": "hodlTaxPct",
            "docs": [
              "Whole-percent HODL tax charged on degenBTC reward withdrawal.",
              "`100` = 100%. Paid by paper hands; redistributed to remaining diamond",
              "hands via `HodlPool::hodl_tax_index` (closed loop — no vault drain)."
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "degenBtcMining",
      "docs": [
        "HashBeast-BTC Mining status and parameters"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "dbtcTokenVault",
            "docs": [
              "Token vault that holds all pre-minted tokens"
            ],
            "type": "pubkey"
          },
          {
            "name": "dbtcPerRound",
            "docs": [
              "degenBTC mined per slot (original base rate)"
            ],
            "type": "u64"
          },
          {
            "name": "totalTokensMined",
            "docs": [
              "Total tokens mined so far"
            ],
            "type": "u64"
          },
          {
            "name": "totalTokensDistributed",
            "docs": [
              "Total tokens distributed so far"
            ],
            "type": "u64"
          },
          {
            "name": "bump",
            "docs": [
              "Bump for PDA derivation"
            ],
            "type": "u8"
          },
          {
            "name": "vaultAuthBump",
            "docs": [
              "Bump for vault authority PDA derivation"
            ],
            "type": "u8"
          },
          {
            "name": "raydiumPoolState",
            "docs": [
              "Raydium pool state for MINE_BTC-SOL trading"
            ],
            "type": "pubkey"
          },
          {
            "name": "lastRateUpdate",
            "docs": [
              "Last time distribution rate was updated (timestamp)"
            ],
            "type": "i64"
          },
          {
            "name": "priceHistory",
            "docs": [
              "Price history for 4-hour rolling average (8 entries, 1 per 30 mins)"
            ],
            "type": {
              "vec": {
                "defined": {
                  "name": "priceEntry"
                }
              }
            }
          },
          {
            "name": "recentPrice",
            "docs": [
              "Recent price (last snapshot, used for comparison)"
            ],
            "type": "u64"
          },
          {
            "name": "trackPrice",
            "docs": [
              "Track price (price when last rate change actually happened)"
            ],
            "type": "u64"
          },
          {
            "name": "polStats",
            "docs": [
              "Protocol Owned Liquidity tracking"
            ],
            "type": {
              "defined": {
                "name": "protocolOwnedLiquidity"
              }
            }
          },
          {
            "name": "lpTokenPriceInSol",
            "docs": [
              "LP token price in SOL (9-decimal precision, updated during oracle updates)"
            ],
            "type": "u64"
          },
          {
            "name": "priceChangeThreshold",
            "docs": [
              "Price change threshold percentage (e.g., 3 = 3%) - rate changes only if price moves beyond this"
            ],
            "type": "u64"
          },
          {
            "name": "emissionIncreasePct",
            "docs": [
              "Emission increase percentage when price goes up (e.g., 1 = 1% increase)"
            ],
            "type": "u64"
          },
          {
            "name": "emissionDecreasePct",
            "docs": [
              "Emission decrease percentage when price goes down (e.g., 3 = 3% decrease)"
            ],
            "type": "u64"
          },
          {
            "name": "lpOperationPending",
            "docs": [
              "Flag indicating LP operation is pending after rate update"
            ],
            "type": "bool"
          }
        ]
      }
    },
    {
      "name": "degenBtcStakingRewardsDistributed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "roundId",
            "type": "u64"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "totalDegenbtcHashpower",
            "docs": [
              "Hashpower denominator used to compute the emitted reward indexes."
            ],
            "type": "u64"
          },
          {
            "name": "dbtcStakerRewards",
            "type": "u64"
          },
          {
            "name": "solStakerRewards",
            "type": "u64"
          },
          {
            "name": "degenbtcDegenbtcRewardIndex",
            "type": "u128"
          },
          {
            "name": "degenbtcSolRewardIndex",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "distributionRateUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oldRate",
            "type": "u64"
          },
          {
            "name": "newRate",
            "type": "u64"
          },
          {
            "name": "priceChangePct",
            "type": "i32"
          },
          {
            "name": "currentPrice",
            "type": "u64"
          },
          {
            "name": "avgPrice4h",
            "type": "u64"
          },
          {
            "name": "trackPrice",
            "type": "u64"
          },
          {
            "name": "recentPrice",
            "type": "u64"
          },
          {
            "name": "rateChanged",
            "type": "bool"
          },
          {
            "name": "newMiningMultiplier",
            "type": "u16"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "evolutionUnlockStageUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "maxEvolutionStageUnlocked",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "factionAdded",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "factionName",
            "type": "string"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "factionKey",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "factionState",
      "docs": [
        "Faction State PDA (Seed: `[b\"faction\", faction_name.as_bytes()]`)",
        "Tracks cumulative statistics and reward indexes for a specific faction.",
        "One account per faction (up to MAX_FACTIONS factions).",
        "Used for calculating staker rewards based on faction performance."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "factionId",
            "docs": [
              "The faction ID (matching index in supported_factions)"
            ],
            "type": "u8"
          },
          {
            "name": "totalDegenbtcHashpower",
            "docs": [
              "Total passive hashpower from stakers in this faction (cumulative)"
            ],
            "type": "u64"
          },
          {
            "name": "degenbtcStaked",
            "type": "u64"
          },
          {
            "name": "degenbtcDegenbtcRewardIndex",
            "type": "u128"
          },
          {
            "name": "degenbtcSolRewardIndex",
            "type": "u128"
          },
          {
            "name": "totalLpHashpower",
            "type": "u64"
          },
          {
            "name": "lpStaked",
            "type": "u64"
          },
          {
            "name": "lpSolRewardIndex",
            "type": "u128"
          },
          {
            "name": "lpDegenbtcRewardIndex",
            "type": "u128"
          },
          {
            "name": "hashbeastsStaked",
            "type": "u64"
          },
          {
            "name": "hashbeastsPlaying",
            "docs": [
              "Total hashbeasts currently being used in gameplay"
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "factionTreasuryRewardsClaimed",
      "docs": [
        "Event emitted when a faction claims treasury rewards for a settled faction_war."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "rank",
            "type": "u8"
          },
          {
            "name": "rewardAmount",
            "type": "u64"
          },
          {
            "name": "dbtcShare",
            "type": "u64"
          },
          {
            "name": "lpShare",
            "type": "u64"
          },
          {
            "name": "rebornAmount",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "factionWarConfig",
      "docs": [
        "Faction War configuration PDA (Seed: `[b\"faction-war-config\"]`)",
        "Faction wars are tied to the economy cycle: one faction war per LP-burn cycle.",
        "Settlement becomes possible once lp_operations_count reaches settle_at_lp_op_count."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "currentWarId",
            "docs": [
              "Current faction-war ID (incrementing counter, starts at 1)"
            ],
            "type": "u64"
          },
          {
            "name": "rewardsSolVaultBump",
            "docs": [
              "Cached PDA bump for `war_sol_vault`. Stored here so the hot",
              "JoinBets path can derive the vault address with `create_program_address`",
              "instead of paying `find_program_address` every bet."
            ],
            "type": "u8"
          },
          {
            "name": "settleAtLpOpCount",
            "docs": [
              "The LP operations count that triggers settlement of the current faction_war.",
              "Set to `pol_stats.lp_operations_count + 1` when the faction_war starts,",
              "meaning the faction_war settles after the next full economy cycle completes."
            ],
            "type": "u32"
          },
          {
            "name": "prevRanks",
            "docs": [
              "Rankings from the previous faction war's gameplay scores.",
              "Used as start_ranks when the next faction war auto-starts.",
              "Initialized to [0, 1, 2, ..., NUM_FACTIONS-1] on first setup."
            ],
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          },
          {
            "name": "lastProcessedRoundId",
            "docs": [
              "Last round whose round-completion side effects were applied to this cycle."
            ],
            "type": "u64"
          },
          {
            "name": "cycleEndRoundId",
            "docs": [
              "Round id snapshotted by the LP-burn instruction when",
              "`lp_operations_count` first reaches `settle_at_lp_op_count`. Marks the",
              "final round of the current cycle — any round after this one belongs to",
              "the next war. `0` while the cycle is still open.",
              "",
              "Lifecycle:",
              "- LP burn captures `global_game_state.current_round_id` here once the",
              "threshold crosses.",
              "- `start_round` is blocked once this is non-zero (war must be settled",
              "before a new round can begin).",
              "- `settle_war` requires this to be non-zero AND",
              "`last_processed_round_id == cycle_end_round_id` (boundary round",
              "already folded into war_state).",
              "- Stays non-zero after `finalize_war_settlement` so `start_round`",
              "remains blocked until `initialize_war_internal` creates the next",
              "war PDAs, then resets to `0` there so the next war starts fresh."
            ],
            "type": "u64"
          },
          {
            "name": "solVolumeSinceLastWin",
            "docs": [
              "Per-country additive SOL volume accumulated since each country's last",
              "round win. Resets to 0 for the winner inside",
              "`track_war_round_completion` AFTER snapshotting onto",
              "`GameSession.winning_faction_volume_at_round`. Persists across cycle",
              "boundaries — a country in a long drought builds up potential."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "miningMultiplierBps",
            "docs": [
              "Current multiplier for faction-war degenBTC rewards, in basis points.",
              "10_000 = 1.0x. Applied to `total_dbtc_mined_in_rounds` at settlement."
            ],
            "type": "u16"
          },
          {
            "name": "multiplierIncreaseBps",
            "docs": [
              "Basis-point increase applied when price goes up (e.g. 300 = +3%)."
            ],
            "type": "u16"
          },
          {
            "name": "multiplierDecreaseBps",
            "docs": [
              "Basis-point decrease applied when price goes down (e.g. 1000 = -10%)."
            ],
            "type": "u16"
          },
          {
            "name": "multiplierMinBps",
            "docs": [
              "Hard floor for the multiplier (min protocol cap: 1000 = 0.1x)."
            ],
            "type": "u16"
          },
          {
            "name": "multiplierMaxBps",
            "docs": [
              "Hard ceiling for the multiplier (max protocol cap: 30000 = 3.0x)."
            ],
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "factionWarMultiplierUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "oldMultiplierBps",
            "type": "u16"
          },
          {
            "name": "newMultiplierBps",
            "type": "u16"
          },
          {
            "name": "direction",
            "type": "i8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "factionWarRewardsClaimed",
      "docs": [
        "Event emitted when a user claims faction_war rewards. dBTC amounts are",
        "per-lane (base + mvp + hb = reward_amount). SOL is broken out the same way",
        "so the indexer can attribute earnings to predict/perform/mvp activity."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "rewardAmount",
            "type": "u64"
          },
          {
            "name": "baseRewardAmount",
            "type": "u64"
          },
          {
            "name": "mvpBonusAmount",
            "type": "u64"
          },
          {
            "name": "hashbeastBonusAmount",
            "type": "u64"
          },
          {
            "name": "solRewardAmount",
            "type": "u64"
          },
          {
            "name": "solBaseAmount",
            "type": "u64"
          },
          {
            "name": "solHbAmount",
            "type": "u64"
          },
          {
            "name": "solMvpAmount",
            "type": "u64"
          },
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "factionWarSettled",
      "docs": [
        "Event emitted when a faction_war is settled.",
        "Rankings driven by on-chain gameplay scores accumulated during the faction_war."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "totalDegenbtcMined",
            "type": "u64"
          },
          {
            "name": "dbtcMinedThisWar",
            "type": "u64"
          },
          {
            "name": "finalRanks",
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          },
          {
            "name": "rankDeltas",
            "type": {
              "array": [
                "i8",
                12
              ]
            }
          },
          {
            "name": "resolvedDirections",
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          },
          {
            "name": "baseRewardPools",
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "hashbeastRewardPools",
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "solBasePool",
            "docs": [
              "SOL lane allocations (sum across eligibles). Per-user SOL share at",
              "claim time scales each by `user_dbtc_lane / total_dbtc_lane`."
            ],
            "type": "u64"
          },
          {
            "name": "solHbPool",
            "type": "u64"
          },
          {
            "name": "solMvpPool",
            "type": "u64"
          },
          {
            "name": "undistributedSol",
            "docs": [
              "SOL drained to sol_treasury because no eligible claimant existed for",
              "that rank/lane slot (e.g. faction had no winners on resolved direction,",
              "no mutations, or no MVP)."
            ],
            "type": "u64"
          },
          {
            "name": "mvpBonus",
            "docs": [
              "Per-faction MVP dBTC bonus (zero where mvp_user[fid] == default)."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "mvpUser",
            "docs": [
              "Per-faction MVP user (default pubkey = no MVP this cycle)."
            ],
            "type": {
              "array": [
                "pubkey",
                12
              ]
            }
          },
          {
            "name": "mvpScore",
            "docs": [
              "Per-faction winning MVP score."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "factionMutationScores",
            "docs": [
              "Per-faction HashBeast mutation-score denominator for HB bonus claims."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "roundWins",
            "type": {
              "array": [
                "u16",
                12
              ]
            }
          },
          {
            "name": "gameplayScores",
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "factionWarSettlement",
      "docs": [
        "Faction War settlement PDA (Seed: `[b\"faction-war-settlement\", war_id_u64_le]`)",
        "Holds all settlement-only data computed when a faction war ends.",
        "Loaded by settle_war and claim_war_rewards — NOT by join_bets or settle_round."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "warId",
            "docs": [
              "FactionWar ID (must match the corresponding FactionWarState)"
            ],
            "type": "u64"
          },
          {
            "name": "finalRanks",
            "docs": [
              "Final ranks derived from the gameplay-score array at settlement."
            ],
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          },
          {
            "name": "rankDeltas",
            "docs": [
              "Rank deltas at settlement (positive = rank improved, negative = rank worsened)."
            ],
            "type": {
              "array": [
                "i8",
                12
              ]
            }
          },
          {
            "name": "resolvedDirections",
            "docs": [
              "Resolved direction per faction (0=Down, 1=Neutral, 2=Up)."
            ],
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          },
          {
            "name": "mvpBonus",
            "docs": [
              "Bonus amount reserved for each faction's MVP at settlement."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "baseRewardPools",
            "docs": [
              "Pre-computed base reward pool per faction (rank-weighted across factions,",
              "then shared by anyone who picked that country's resolved direction correctly)."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "hashbeastRewardPools",
            "docs": [
              "Reward pool per faction reserved for gameplay HashBeasts backing their home country during the faction_war."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "solBasePool",
            "docs": [
              "SOL lane allocations (mirror of the dBTC lanes — same bps). Per-user",
              "SOL payout at claim time scales each lane's pool by the user's dBTC",
              "share of that lane:",
              "user_sol_<lane> = sol_<lane>_pool * user_dbtc_<lane> / total_dbtc_<lane>",
              "",
              "Distribution is **absolute rank-weighted**: each active faction's slice",
              "of every lane is determined by its rank weight relative to the sum of",
              "rank weights across all active factions. Non-eligible factions' slices",
              "are NOT redistributed to other factions; they stay unallocated.",
              "",
              "`sol_base_pool + sol_hb_pool + sol_mvp_pool + undistributed_sol ==",
              "FactionWarState.sol_reward_pool` at settle time."
            ],
            "type": "u64"
          },
          {
            "name": "solHbPool",
            "type": "u64"
          },
          {
            "name": "solMvpPool",
            "type": "u64"
          },
          {
            "name": "undistributedSol",
            "docs": [
              "SOL that no eligible claimant can claim (no faction met the lane's",
              "eligibility rule, or the rank-weight slot belonged to a faction with",
              "no eligibles). Transferred to `sol_treasury` at settle so it doesn't",
              "rot in the faction-war SOL vault."
            ],
            "type": "u64"
          },
          {
            "name": "treasuryClaimedBitmap",
            "docs": [
              "Bitmap of factions that have already claimed treasury rewards for this",
              "faction war. Bit N = 1 means faction N has claimed."
            ],
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "factionWarStarted",
      "docs": [
        "Emitted by `initialize_war_internal` when a new cycle's FactionWarState",
        "PDA is created. Lets indexers detect cycle starts without scanning ix data."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "factionCount",
            "type": "u8"
          },
          {
            "name": "startTimestamp",
            "type": "u64"
          },
          {
            "name": "prevRanks",
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          },
          {
            "name": "settleAtLpOpCount",
            "docs": [
              "LP-operations count threshold; once `pol_stats.lp_operations_count`",
              "reaches this, the LP-burn ix snapshots the cycle's final round."
            ],
            "type": "u32"
          },
          {
            "name": "treasuryRewardBaseAmount",
            "docs": [
              "Treasury-tax SOL seeded at war start (rolled forward from",
              "`tax_config.unassigned_war_treasury_amount`)."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "factionWarState",
      "docs": [
        "Faction War state PDA (Seed: `[b\"faction-war\", war_id_u64_le]`)",
        "Tracks active gameplay data during a faction war cycle.",
        "Kept small because it is loaded on every bet and every settle_round."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "warId",
            "docs": [
              "FactionWar ID"
            ],
            "type": "u64"
          },
          {
            "name": "startTimestamp",
            "docs": [
              "Timestamp when this faction_war was started"
            ],
            "type": "u64"
          },
          {
            "name": "stage",
            "docs": [
              "Stage: 0 = active, 1 = settled (claims open)"
            ],
            "type": "u8"
          },
          {
            "name": "factionCount",
            "docs": [
              "Snapshot of how many factions were active when this faction_war started"
            ],
            "type": "u8"
          },
          {
            "name": "totalDbtcMinedInRounds",
            "docs": [
              "Total degenBTC mined via raffle rounds during this faction_war."
            ],
            "type": "u64"
          },
          {
            "name": "dbtcMinedThisWar",
            "docs": [
              "Faction-war mining pool distributed to faction-war predictors."
            ],
            "type": "u64"
          },
          {
            "name": "factionDirectionTotals",
            "docs": [
              "Total weighted bets per faction and direction during this faction_war",
              "from all users. This powers the base \"be right anywhere\" cycle rewards."
            ],
            "type": {
              "array": [
                {
                  "array": [
                    "u64",
                    3
                  ]
                },
                12
              ]
            }
          },
          {
            "name": "roundWins",
            "docs": [
              "Number of raffle rounds won by each faction during this faction war.",
              "Used as a tiebreak after story score."
            ],
            "type": {
              "array": [
                "u16",
                12
              ]
            }
          },
          {
            "name": "totalCycleSol",
            "docs": [
              "Total real SOL volume across all factions/directions for this war.",
              "Folded once per round from `game_session.total_sol_bets`. Used as the",
              "`total_sol` denominator in the claim-time mutation chance roll."
            ],
            "type": "u64"
          },
          {
            "name": "gameplayScores",
            "docs": [
              "Accumulated gameplay scores per faction during this faction_war.",
              "Drives ranking at settlement (round wins is the tiebreak, then faction_id)."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "mvpUser",
            "docs": [
              "Running MVP candidate per faction (user with highest cumulative gameplay score)."
            ],
            "type": {
              "array": [
                "pubkey",
                12
              ]
            }
          },
          {
            "name": "mvpScore",
            "docs": [
              "Running MVP score per faction."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "factionMutationScore",
            "docs": [
              "Total mutation-bonus score per faction across all users this cycle.",
              "Incremented in `apply_mutation_bonus_score` alongside per-user totals.",
              "Denominator for HB bonus claim share — `hb_share[user] =",
              "hb_pool[home] * user_mutation_score / faction_mutation_score[home]`.",
              "HB lane is now purely gameplay-driven: you must have rolled at least",
              "one successful mutation this cycle to earn HB-bonus."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "solRewardPool",
            "docs": [
              "Accumulated SOL (from `cycle_sol_split`) reserved for this faction-war",
              "cycle's SOL jackpot. Distributed to claimants at settlement."
            ],
            "type": "u64"
          },
          {
            "name": "treasuryRewardBaseAmount",
            "docs": [
              "Exact amount of faction treasury tax attributed to this faction war.",
              "Accumulated during tax distribution while the war is active, or",
              "seeded from TaxConfig.unassigned_war_treasury_amount when the",
              "war state is first initialized."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "factionsConfig",
      "docs": [
        "Autominer configuration for factions"
      ],
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "specific",
            "fields": [
              {
                "name": "picks",
                "type": {
                  "vec": {
                    "defined": {
                      "name": "autominerFactionPick"
                    }
                  }
                }
              }
            ]
          },
          {
            "name": "random",
            "fields": [
              {
                "name": "count",
                "type": "u8"
              },
              {
                "name": "direction",
                "type": {
                  "defined": {
                    "name": "predictionDirection"
                  }
                }
              }
            ]
          }
        ]
      }
    },
    {
      "name": "floorEntry",
      "docs": [
        "One entry in the on-chain sorted-floor queue. Tracks a user-listed asset",
        "(program-owned listings are explicitly excluded — sweep buying the",
        "protocol's own listings would be circular). Stale entries (listing was",
        "canceled directly via the marketplace, bypassing our `cancel_user_listing`",
        "wrapper that would have atomic-deregistered) are popped one at a time",
        "by `sweep_floor_lowest`; the keeper bounty for that purge is the",
        "`STALE_PURGE_KEEPER_REWARD_LAMPORTS` constant, set deliberately low to",
        "defuse list→raw-cancel→purge spam attacks. See that constant's docs."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "listing",
            "type": "pubkey"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "seller",
            "type": "pubkey"
          },
          {
            "name": "price",
            "type": "u64"
          },
          {
            "name": "registeredAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "floorEntryRegistered",
      "docs": [
        "A user (or keeper) registered a marketplace listing into the floor queue."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "listing",
            "type": "pubkey"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "seller",
            "type": "pubkey"
          },
          {
            "name": "price",
            "type": "u64"
          },
          {
            "name": "queueIndex",
            "type": "u8"
          },
          {
            "name": "queueSizeAfter",
            "type": "u8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "floorEntryRemoved",
      "docs": [
        "A floor queue entry was removed (sale, cancel, price-update reorder, or stale)."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "listing",
            "type": "pubkey"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "queueIndex",
            "type": "u8"
          },
          {
            "name": "reason",
            "type": "u8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "floorHistory",
      "docs": [
        "Singleton 7-entry rolling buffer of daily floor anchors.",
        "`compute_trend_bps` reads the head vs. the (head+1) wrap-around to get a",
        "7-day delta in basis points.",
        "Seeds: `[b\"floor-history\"]`."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "head",
            "type": "u8"
          },
          {
            "name": "lastSnapshotAt",
            "type": "i64"
          },
          {
            "name": "snapshots",
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "floorSnapshot"
                  }
                },
                7
              ]
            }
          }
        ]
      }
    },
    {
      "name": "floorQueue",
      "docs": [
        "Singleton sorted-ascending queue of the cheapest user listings.",
        "Invariants: `entries[..entries_count]` is sorted ascending by `price`.",
        "Seeds: `[b\"floor-queue\"]`."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "entriesCount",
            "type": "u8"
          },
          {
            "name": "entries",
            "type": {
              "array": [
                {
                  "defined": {
                    "name": "floorEntry"
                  }
                },
                20
              ]
            }
          }
        ]
      }
    },
    {
      "name": "floorSnapshot",
      "docs": [
        "One entry in the 7-day rolling floor snapshot ringbuffer."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "timestamp",
            "type": "i64"
          },
          {
            "name": "anchorPrice",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "floorSnapshotRecorded",
      "docs": [
        "A daily floor snapshot was committed."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "anchorPrice",
            "type": "u64"
          },
          {
            "name": "source",
            "docs": [
              "0 = sale median, 1 = queue median fallback, 2 = sale capped by queue,",
              "3 = first snapshot capped to marketplace min, 4 = capped by prior anchor."
            ],
            "type": "u8"
          },
          {
            "name": "samples",
            "type": "u32"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "floorSweepExecuted",
      "docs": [
        "Inventory PDA bought the cheapest user-listed NFT via `sweep_floor_lowest`.",
        "Disposition is reflected in a follow-up event (LootboxQueuePush,",
        "InventoryAssetRelisted, or InventoryAssetBurned)."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "buyPrice",
            "type": "u64"
          },
          {
            "name": "seller",
            "type": "pubkey"
          },
          {
            "name": "anchorPrice",
            "type": "u64"
          },
          {
            "name": "trendBps",
            "type": "i32"
          },
          {
            "name": "staleSkipped",
            "type": "u8"
          },
          {
            "name": "keeper",
            "type": "pubkey"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "gamePauseToggled",
      "docs": [
        "Emitted when the authority toggles the global pause flag.",
        "Indexers should propagate `is_paused` to the frontend so the UI can",
        "disable bet/mint actions and show a clear \"paused\" banner to users."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "isPaused",
            "type": "bool"
          },
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "gameSession",
      "docs": [
        "Game Session PDA (Seed: `[b\"game-session\", round_id_u64]`)",
        "Each round has its own GameSession PDA that tracks:",
        "- Round timing (start/end timestamps)",
        "- Total bets placed in this round",
        "- Per-faction indexes for tracking individual bets",
        "- Winning faction",
        "- Round-specific reward pools and payout data",
        "This account is created when a round starts and finalized when the round ends."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "stage",
            "type": "u8"
          },
          {
            "name": "roundId",
            "docs": [
              "The round ID this session belongs to"
            ],
            "type": "u64"
          },
          {
            "name": "roundStartSlot",
            "docs": [
              "Slot when the round started."
            ],
            "type": "u64"
          },
          {
            "name": "roundStartTimestamp",
            "type": "i64"
          },
          {
            "name": "roundEndTimestamp",
            "docs": [
              "Timestamp after which betting is closed."
            ],
            "type": "i64"
          },
          {
            "name": "scheduledEntropySlot",
            "docs": [
              "Primary future slot whose hash should be used as round entropy."
            ],
            "type": "u64"
          },
          {
            "name": "entropySlotUsed",
            "docs": [
              "Actual slot whose hash was used to derive the winner."
            ],
            "type": "u64"
          },
          {
            "name": "entropyHash",
            "docs": [
              "Stored slot hash used for winner derivation."
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "usedEntropyFallback",
            "docs": [
              "Whether the round had to fall back to latest-available slot hash instead of the scheduled one."
            ],
            "type": "bool"
          },
          {
            "name": "totalSolBets",
            "docs": [
              "Total SOL bets placed in this round"
            ],
            "type": "u64"
          },
          {
            "name": "totalPointsBets",
            "docs": [
              "Total points bets placed in this round"
            ],
            "type": "u64"
          },
          {
            "name": "totalWgtdPointsBets",
            "docs": [
              "Total weighted points bets (for degenBTC distribution)"
            ],
            "type": "u64"
          },
          {
            "name": "stakersFee",
            "docs": [
              "Total stakers fee paid in this round"
            ],
            "type": "u64"
          },
          {
            "name": "cycleSolPool",
            "docs": [
              "SOL added to the war's cycle-SOL pot this round (sum of",
              "`cycle_sol_split_per_bet × num_bets` across every bet in this round).",
              "Folded into `war_state.sol_reward_pool` at settle_round via",
              "`track_war_round_completion`. Lets us track the cycle SOL pool without",
              "loading war_state on the bet hot path."
            ],
            "type": "u64"
          },
          {
            "name": "userFactionIndexes",
            "docs": [
              "Number of users who bet on each faction."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "solBetsByFaction",
            "docs": [
              "Net SOL bet placed on each faction."
            ],
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "pointsBetsByFactionDirection",
            "docs": [
              "Points bet placed on each faction-direction pair."
            ],
            "type": {
              "array": [
                {
                  "array": [
                    "u64",
                    3
                  ]
                },
                12
              ]
            }
          },
          {
            "name": "wgtdPointsBetsByFactionDirection",
            "docs": [
              "Weighted points bet placed on each faction-direction pair."
            ],
            "type": {
              "array": [
                {
                  "array": [
                    "u64",
                    3
                  ]
                },
                12
              ]
            }
          },
          {
            "name": "winningFactionId",
            "docs": [
              "The winning faction ID for this round."
            ],
            "type": "u8"
          },
          {
            "name": "winningDirection",
            "docs": [
              "The winning direction for the winning faction (0=Down, 1=Neutral, 2=Up)."
            ],
            "type": "u8"
          },
          {
            "name": "dbtcWinnerPool",
            "docs": [
              "degenBTC allocated for exact winning faction+direction bettors in this round."
            ],
            "type": "u64"
          },
          {
            "name": "dbtcSameFactionDirectionPools",
            "docs": [
              "degenBTC allocated per losing direction on the winning faction.",
              "The winning direction index remains zero in this array."
            ],
            "type": {
              "array": [
                "u64",
                3
              ]
            }
          },
          {
            "name": "factionStakers",
            "docs": [
              "degenBTC allocated for stakers in this round"
            ],
            "type": "u64"
          },
          {
            "name": "jackpotRewards",
            "docs": [
              "degenBTC allocated for the global jackpot in this round."
            ],
            "type": "u64"
          },
          {
            "name": "solRewardsIndex",
            "docs": [
              "SOL rewards index for this round's exact winning faction+direction."
            ],
            "type": "u128"
          },
          {
            "name": "dbtcRewardsIndex",
            "docs": [
              "degenBTC rewards index for this round's exact winning faction+direction."
            ],
            "type": "u128"
          },
          {
            "name": "jackpotHit",
            "docs": [
              "Whether the global jackpot was hit in this round."
            ],
            "type": "bool"
          },
          {
            "name": "jackpotFactionId",
            "docs": [
              "The faction ID that wins the global jackpot this round (if hit)."
            ],
            "type": "u8"
          },
          {
            "name": "jackpotPotSizeOnHit",
            "docs": [
              "Global jackpot pot size when hit (if applicable)."
            ],
            "type": "u64"
          },
          {
            "name": "jackpotRewardsIndex",
            "docs": [
              "degenBTC rewards index for jackpot winners (all directions on jackpot faction).",
              "Set during `settle_round`; read by `claim_round_rewards`."
            ],
            "type": "u128"
          },
          {
            "name": "mutationsPerFaction",
            "docs": [
              "Number of mutations that have occurred per faction this round.",
              "More mutations in a faction → harder for the next one (diminishing returns)."
            ],
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          },
          {
            "name": "totalMutationsThisRound",
            "docs": [
              "Total mutations across all factions this round.",
              "Capped at active_factions / 3 to create scarcity."
            ],
            "type": "u8"
          },
          {
            "name": "warIdWhenPlayed",
            "docs": [
              "Snapshot of `war_config.current_war_id` at round start.",
              "Used by the round-claim handler to detect late claims (cycle has settled",
              "after the round ended) so mutation-bonus score is dropped instead of",
              "being applied to a different cycle."
            ],
            "type": "u64"
          },
          {
            "name": "winningFactionVolumeAtRound",
            "docs": [
              "Snapshot of the winning country's `sol_volume_since_last_win`",
              "captured at round-end (in `track_war_round_completion`),",
              "BEFORE the config counter is reset to 0. Frozen value the round-claim",
              "mutation roll feeds into the volume_factor — late claims see the same",
              "number even though the config-side counter has long been reset."
            ],
            "type": "u64"
          },
          {
            "name": "solRewardPoolAccumulated",
            "docs": [
              "Accumulated cycle SOL split from bets placed during this round.",
              "Folded into `FactionWarState.sol_reward_pool` once per round at",
              "`settle_round` rather than touched per-bet — keeps JoinBets fast."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "gameplayScoreAccumulated",
      "docs": [
        "Cycle leaderboard score-add event. Emitted from two sites:",
        "",
        "- `score_source = GAMEPLAY_SCORE_SOURCE_ROUND_WIN (0)`: end-of-round",
        "accumulation when a country wins. `score_added` equals the round's",
        "total weighted points bet on that country (any direction).",
        "",
        "- `score_source = GAMEPLAY_SCORE_SOURCE_JACKPOT_HIT (2)`: jackpot",
        "accumulation when the independently selected jackpot country actually",
        "receives the pot. `score_added` equals the round's total weighted points",
        "bet on that jackpot country (any direction).",
        "",
        "- `score_source = GAMEPLAY_SCORE_SOURCE_MUTATION_BONUS (1)`: per-claim",
        "bonus when a player's round-claim mutation roll succeeds and the",
        "round's cycle is still active. Full bonus is",
        "`user_wgtd_points_on_winner × active_multiplier / BASE_MULTIPLIER × mutation_weight`",
        "where `mutation_weight` is 4/2/1 for Evolution/Power/Trait.",
        "`score_added` reflects the **leaderboard delta actually applied**:",
        "the full bonus if the user backed their own home faction, or **half**",
        "the bonus if the user was playing mercenary on a foreign winner.",
        "Only home-win mutations also affect MVP candidacy and HB-bonus pools."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "scoreSource",
            "type": "u8"
          },
          {
            "name": "scoreAdded",
            "type": "u64"
          },
          {
            "name": "factionTotalScore",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "gameplayTuningConfig",
      "docs": [
        "Unified gameplay tuning stored directly inside `GlobalConfig`."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "rpgProgression",
            "docs": [
              "Enable RPG progression (story events, XP, etc) during gameplay."
            ],
            "type": "bool"
          },
          {
            "name": "maxEvolutionStageUnlocked",
            "docs": [
              "Highest evolution stage currently unlocked by admin.",
              "`0` disables evolutions entirely, `1` allows stage 0 -> 1, etc."
            ],
            "type": "u8"
          },
          {
            "name": "warBaseRewardBps",
            "docs": [
              "Faction-war mining pool split in basis points. Must sum to 10_000."
            ],
            "type": "u16"
          },
          {
            "name": "warMvpRewardBps",
            "type": "u16"
          },
          {
            "name": "warHashbeastRewardBps",
            "type": "u16"
          },
          {
            "name": "baseMutationChanceBps",
            "docs": [
              "Baseline mutation chance before runtime factors."
            ],
            "type": "u16"
          },
          {
            "name": "mutationChanceFloorBps",
            "docs": [
              "Final chance floor / cap after all runtime factors are applied."
            ],
            "type": "u16"
          },
          {
            "name": "mutationChanceCapBps",
            "type": "u16"
          },
          {
            "name": "factionVolumeThresholdLamports",
            "docs": [
              "Per-faction additive volume controller. Required volume base (and",
              "per-mutation ramp) for the country's accumulated SOL bets since its",
              "last round win to fully unlock the volume_factor in the chance formula."
            ],
            "type": "u64"
          },
          {
            "name": "extraVolumeThresholdPerMutationLamports",
            "type": "u64"
          },
          {
            "name": "targetMutationsPerCycle",
            "docs": [
              "Cycle pacing controller. Pacing factor alone regulates how many",
              "mutations land per cycle — it's a closed-loop controller comparing",
              "observed-vs-target. No separate cooldown controller needed."
            ],
            "type": "u16"
          },
          {
            "name": "targetRoundsPerCycle",
            "type": "u16"
          },
          {
            "name": "pacingMaxAdjustmentBps",
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "gameplayTuningUpdateArgs",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "enableRpgProgression",
            "type": {
              "option": "bool"
            }
          },
          {
            "name": "maxEvolutionStageUnlocked",
            "type": {
              "option": "u8"
            }
          },
          {
            "name": "warBaseRewardBps",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "warMvpRewardBps",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "warHashbeastRewardBps",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "baseMutationChanceBps",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "mutationChanceFloorBps",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "mutationChanceCapBps",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "factionVolumeThresholdLamports",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "extraVolumeThresholdPerMutationLamports",
            "type": {
              "option": "u64"
            }
          },
          {
            "name": "targetMutationsPerCycle",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "targetRoundsPerCycle",
            "type": {
              "option": "u16"
            }
          },
          {
            "name": "pacingMaxAdjustmentBps",
            "type": {
              "option": "u16"
            }
          }
        ]
      }
    },
    {
      "name": "gameplayTuningUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "rpgProgression",
            "type": "bool"
          },
          {
            "name": "maxEvolutionStageUnlocked",
            "type": "u8"
          },
          {
            "name": "warBaseRewardBps",
            "type": "u16"
          },
          {
            "name": "warMvpRewardBps",
            "type": "u16"
          },
          {
            "name": "warHashbeastRewardBps",
            "type": "u16"
          },
          {
            "name": "baseMutationChanceBps",
            "type": "u16"
          },
          {
            "name": "mutationChanceFloorBps",
            "type": "u16"
          },
          {
            "name": "mutationChanceCapBps",
            "type": "u16"
          },
          {
            "name": "factionVolumeThresholdLamports",
            "type": "u64"
          },
          {
            "name": "extraVolumeThresholdPerMutationLamports",
            "type": "u64"
          },
          {
            "name": "targetMutationsPerCycle",
            "type": "u16"
          },
          {
            "name": "targetRoundsPerCycle",
            "type": "u16"
          },
          {
            "name": "pacingMaxAdjustmentBps",
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "globalConfig",
      "docs": [
        "",
        "------------ GLOBAL CONFIG ------------",
        "Global configuration for the program"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "totalPlayers",
            "docs": [
              "total number of players in the game"
            ],
            "type": "u64"
          },
          {
            "name": "extAuthority",
            "docs": [
              "Authority that can update config parameters"
            ],
            "type": "pubkey"
          },
          {
            "name": "pendingAuthority",
            "docs": [
              "Pending authority for 2-step transfer (Pubkey::default() = no pending transfer)"
            ],
            "type": "pubkey"
          },
          {
            "name": "feeRecipient",
            "docs": [
              "Direct recipient for hashbeast mints + dev earnings revenue"
            ],
            "type": "pubkey"
          },
          {
            "name": "pdaSolTreasury",
            "docs": [
              "PDA account that holds collected SOL fees"
            ],
            "type": "pubkey"
          },
          {
            "name": "supportedFactions",
            "docs": [
              "List of supported factions (e.g., \"USA\", \"China\", \"Russia\")",
              "Maximum 12 factions, each with max 16 characters"
            ],
            "type": {
              "vec": "string"
            }
          },
          {
            "name": "solFeeConfig",
            "docs": [
              "SOL fee distribution configuration"
            ],
            "type": {
              "defined": {
                "name": "solFeeConfig"
              }
            }
          },
          {
            "name": "dbtcDistConfig",
            "docs": [
              "degenBTC distribution configuration"
            ],
            "type": {
              "defined": {
                "name": "degenBtcDistConfig"
              }
            }
          },
          {
            "name": "raydiumPoolState",
            "docs": [
              "Authorized Raydium pool state address (security: prevents using malicious pools)"
            ],
            "type": "pubkey"
          },
          {
            "name": "snapshotInterval",
            "docs": [
              "Minimum time interval between price snapshots (in seconds)",
              "Default: 1800 seconds (30 minutes)"
            ],
            "type": "u64"
          },
          {
            "name": "gameplayTuning",
            "docs": [
              "Unified gameplay and cycle-reward tuning surface."
            ],
            "type": {
              "defined": {
                "name": "gameplayTuningConfig"
              }
            }
          },
          {
            "name": "isPaused",
            "docs": [
              "Authority-toggleable global pause. When true, blocks: new bets (manual",
              "+ autominer), new round starts, hashbeast mints, and hashbeast breeds. Does NOT",
              "block: round settlement, all claims, staking/unstaking, economy cranks.",
              "Users can always exit; pending rounds always finish."
            ],
            "type": "bool"
          },
          {
            "name": "bump",
            "docs": [
              "------------------------------------------------------------",
              "Bump for GlobalConfig PDA derivation"
            ],
            "type": "u8"
          },
          {
            "name": "treasuryBump",
            "docs": [
              "Bump for SOL treasury PDA derivation"
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "globalGameSate",
      "docs": [
        "Global game state PDA (Seed: `[b\"global-game-state\"]`)",
        "Tracks global game statistics and the currently active round.",
        "Each individual round has its own GameSession PDA."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "isActive",
            "docs": [
              "Whether the game is currently active"
            ],
            "type": "bool"
          },
          {
            "name": "canBeginRound",
            "type": "bool"
          },
          {
            "name": "currentRoundId",
            "docs": [
              "The currently active round ID (e.g., 48636)."
            ],
            "type": "u64"
          },
          {
            "name": "roundDurationSeconds",
            "docs": [
              "Round duration in seconds (configurable)"
            ],
            "type": "i64"
          },
          {
            "name": "lastRoundId",
            "docs": [
              "The last completed round ID"
            ],
            "type": "u64"
          },
          {
            "name": "jackpotPot",
            "docs": [
              "Global jackpot pot that accumulates across all rounds and factions.",
              "When the jackpot hits, this pot is distributed to any-direction bettors on the selected faction."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "hashBeastBred",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "breeder",
            "type": "pubkey"
          },
          {
            "name": "mom",
            "type": "pubkey"
          },
          {
            "name": "dad",
            "type": "pubkey"
          },
          {
            "name": "offspring",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "rebirthCount",
            "type": "u8"
          },
          {
            "name": "pairPriceLamports",
            "type": "u64"
          },
          {
            "name": "floorAnchorLamports",
            "type": "u64"
          },
          {
            "name": "floorMinPriceLamports",
            "type": "u64"
          },
          {
            "name": "totalPriceLamports",
            "type": "u64"
          },
          {
            "name": "solPaidLamports",
            "type": "u64"
          },
          {
            "name": "solFeeRecipientLamports",
            "type": "u64"
          },
          {
            "name": "solTreasuryLamports",
            "type": "u64"
          },
          {
            "name": "dbtcPriceLamports",
            "type": "u64"
          },
          {
            "name": "dbtcPaid",
            "type": "u64"
          },
          {
            "name": "dbtcBurned",
            "type": "u64"
          },
          {
            "name": "dbtcToVault",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "hashBeastCollectionCreated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "collection",
            "type": "pubkey"
          },
          {
            "name": "updateAuthority",
            "type": "pubkey"
          },
          {
            "name": "name",
            "type": "string"
          },
          {
            "name": "uri",
            "type": "string"
          }
        ]
      }
    },
    {
      "name": "hashBeastConfig",
      "docs": [
        "Global HashBeast configuration used outside the primary mint sale.",
        "",
        "Seeds: `[b\"hashbeast-config\"]`. Singleton. Initialized once at deploy",
        "(admin path) and mutated only via admin ix going forward.",
        "",
        "**No lifetime supply cap.** Only the genesis sale is bounded (see",
        "`HashBeastMintConfig.genesis_mint_limit`). Post-genesis, HashBeasts mint",
        "via breeding without a hard ceiling; parent breed-count pricing plus the",
        "floor guard make additional supply progressively expensive.",
        "",
        "**`hashbeast_collection` is the trust anchor** for \"this asset is a",
        "canonical HashBeast\" — every mint/breed Accounts struct address-pins the",
        "`hashbeast_collection` field to this pubkey, every stake/use/withdraw",
        "path resolves the same constraint. Don't add a new entry point that",
        "touches a HashBeast asset without binding the collection. See",
        "`instructions/hashbeasts.rs` module docs for the full mint-flow guard."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "hashbeastCollection",
            "docs": [
              "Canonical Metaplex Core collection address for HashBeasts. Set once",
              "at admin init; mint paths refuse any other collection."
            ],
            "type": "pubkey"
          },
          {
            "name": "totalHashbeastsMinted",
            "docs": [
              "Lifetime count of HashBeasts ever minted. Burns do NOT decrement this."
            ],
            "type": "u64"
          },
          {
            "name": "breedingAllowed",
            "docs": [
              "Admin kill-switch. When false, `breed_hashbeasts` reverts."
            ],
            "type": "bool"
          },
          {
            "name": "breedParentPricesLamports",
            "docs": [
              "Per-parent prices by current breed_count. Valid indexes are 0..=4.",
              "Pair price = mom table price + dad table price, then the floor guard is",
              "applied. Admin-updatable so governance can retune the sink."
            ],
            "type": {
              "array": [
                "u64",
                5
              ]
            }
          }
        ]
      }
    },
    {
      "name": "hashBeastEvolution",
      "docs": [
        "Event emitted when a hashbeast evolves to a new stage"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "origin",
            "docs": [
              "0 = round claim, 1 = faction-war claim."
            ],
            "type": "u8"
          },
          {
            "name": "originId",
            "type": "u64"
          },
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "newStage",
            "type": "u8"
          },
          {
            "name": "visualTraitIndex",
            "docs": [
              "Visual trait mutation that happened during evolution"
            ],
            "type": "u8"
          },
          {
            "name": "visualOldVal",
            "type": "u8"
          },
          {
            "name": "visualNewVal",
            "type": "u8"
          },
          {
            "name": "powerTraitIndex",
            "docs": [
              "Power trait mutation that happened during evolution"
            ],
            "type": "u8"
          },
          {
            "name": "powerOldVal",
            "type": "u8"
          },
          {
            "name": "powerNewVal",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "hashBeastFreeMintAllowance",
      "docs": [
        "Per-user whitelist allowance for free HashBeast mints.",
        "The whitelisted user still pays transaction/account rent, but not the mint fee."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "remainingFreeMints",
            "type": "u8"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "hashBeastFreeMintAllowanceUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "type": "pubkey"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "remainingFreeMints",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "hashBeastGameplayUnlockRequested",
      "docs": [
        "Event emitted when a user requests gameplay unlock for the next faction_war cycle."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "requestedDuringWarId",
            "type": "u64"
          },
          {
            "name": "unlockAvailableAfterWarId",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "hashBeastMetadata",
      "docs": [
        "HashBeast NFT metadata (stored in degenBTC program for simplicity)"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "mint",
            "docs": [
              "The NFT mint address (Metaplex Core asset)"
            ],
            "type": "pubkey"
          },
          {
            "name": "mom",
            "docs": [
              "Parent 1 mint (Pubkey::default() for genesis hashbeasts)"
            ],
            "type": "pubkey"
          },
          {
            "name": "dad",
            "docs": [
              "Parent 2 mint (Pubkey::default() for genesis hashbeasts)"
            ],
            "type": "pubkey"
          },
          {
            "name": "breedCount",
            "docs": [
              "Number of times this hashbeast has bred (max 5)"
            ],
            "type": "u8"
          },
          {
            "name": "rebirthCount",
            "docs": [
              "Number of times this asset has been reborn/reborn (max 7)"
            ],
            "type": "u8"
          },
          {
            "name": "cooldownEnd",
            "docs": [
              "Unix timestamp when cooldown ends (can breed again after this)"
            ],
            "type": "i64"
          },
          {
            "name": "createdAt",
            "docs": [
              "Creation timestamp"
            ],
            "type": "i64"
          },
          {
            "name": "factionId",
            "docs": [
              "Faction ID (country) that the hashbeast belongs to (matches degenBTC faction)"
            ],
            "type": "u8"
          },
          {
            "name": "multiplier",
            "docs": [
              "Multiplier for this hashbeast (1000 = 1x, same scale as BASE_MULTIPLIER)"
            ],
            "type": "u32"
          },
          {
            "name": "accumulatedVal",
            "docs": [
              "degenBTC accumulated which can be claimed by rebirthing this hashbeast"
            ],
            "type": "u64"
          },
          {
            "name": "dna",
            "docs": [
              "DNA data (32 bytes for breeding/evolution)"
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "incubatedPlayerData",
            "docs": [
              "The Player who is incubating this hashbeast. Pubkey::default() if not incubated."
            ],
            "type": "pubkey"
          },
          {
            "name": "lastUpdateTs",
            "docs": [
              "Last power update timestamp"
            ],
            "type": "i64"
          },
          {
            "name": "xp",
            "docs": [
              "Experience points, reset to 0 on evolution"
            ],
            "type": "u32"
          },
          {
            "name": "bump",
            "docs": [
              "PDA bump"
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "hashBeastMintConfig",
      "docs": [
        "Mint-only HashBeast configuration for the genesis sale and free/admin genesis mints.",
        "Non-mint gameplay/staking/breeding instructions should not require this account."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "isActive",
            "docs": [
              "Whether primary genesis minting is currently active."
            ],
            "type": "bool"
          },
          {
            "name": "basePrice",
            "docs": [
              "Base price for the genesis bonding curve (in lamports)."
            ],
            "type": "u64"
          },
          {
            "name": "curveA",
            "docs": [
              "Curve steepness parameter for genesis mint pricing."
            ],
            "type": "u64"
          },
          {
            "name": "genesisMintLimit",
            "docs": [
              "Total number of genesis mints allowed across all factions."
            ],
            "type": "u64"
          },
          {
            "name": "genesisMints",
            "docs": [
              "Number of genesis mints completed so far."
            ],
            "type": "u64"
          },
          {
            "name": "maxGenesisMintsPerFaction",
            "docs": [
              "Max genesis mints allowed per faction/country."
            ],
            "type": "u16"
          },
          {
            "name": "genesisMintsByFaction",
            "docs": [
              "Genesis mints completed per faction/country."
            ],
            "type": {
              "array": [
                "u16",
                12
              ]
            }
          },
          {
            "name": "ticketTiers",
            "docs": [
              "Available ticket tier configs users can choose when minting."
            ],
            "type": {
              "vec": {
                "defined": {
                  "name": "ticketTier"
                }
              }
            }
          }
        ]
      }
    },
    {
      "name": "hashBeastMinted",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "hashbeastMetadataAccount",
            "type": "pubkey"
          },
          {
            "name": "hashbeastAssetSigner",
            "type": "pubkey"
          },
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "player",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "name",
            "type": "string"
          },
          {
            "name": "uri",
            "type": "string"
          },
          {
            "name": "dna",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "multiplier",
            "type": "u32"
          },
          {
            "name": "accumulatedVal",
            "type": "u64"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "price",
            "type": "u64"
          },
          {
            "name": "ticketTier",
            "type": "u64"
          },
          {
            "name": "ticketCount",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "hashBeastPowerMutation",
      "docs": [
        "Event emitted when a hashbeast's power trait is mutated"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "origin",
            "docs": [
              "0 = round claim, 1 = faction-war claim."
            ],
            "type": "u8"
          },
          {
            "name": "originId",
            "type": "u64"
          },
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "traitIndex",
            "type": "u8"
          },
          {
            "name": "oldVal",
            "type": "u8"
          },
          {
            "name": "newVal",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "hashBeastRebirthBurned",
      "docs": [
        "A `rebirth_hashbeast` call burned the asset because the country queue was full,",
        "inventory was full, or the asset had already reached MAX_REBIRTH_COUNT.",
        "User still received their `accumulated_val` payout; the asset is gone."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "formerOwner",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "accumulatedVal",
            "type": "u64"
          },
          {
            "name": "rebirthCount",
            "type": "u8"
          },
          {
            "name": "reason",
            "docs": [
              "0 = queue/inventory full, 1 = max rebirth count reached"
            ],
            "type": "u8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "hashBeastReborn",
      "docs": [
        "Event emitted when a HashBeast is reborn. The user",
        "receives any accumulated_val, then the same asset is reborn into inventory",
        "with fresh DNA and default gameplay state."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "formerOwner",
            "type": "pubkey"
          },
          {
            "name": "accumulatedVal",
            "type": "u64"
          },
          {
            "name": "qualityScore",
            "type": "u16"
          },
          {
            "name": "rebirthCount",
            "type": "u8"
          },
          {
            "name": "newDna",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "hashBeastStaked",
      "docs": [
        "Event emitted when a HashBeast is staked",
        "Tracks multiplier changes and hashpower updates for indexing"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "docs": [
              "User who staked the hashbeast"
            ],
            "type": "pubkey"
          },
          {
            "name": "player",
            "docs": [
              "Player data account address"
            ],
            "type": "pubkey"
          },
          {
            "name": "hashbeastMint",
            "docs": [
              "HashBeast mint address"
            ],
            "type": "pubkey"
          },
          {
            "name": "hashbeastMetadataAccount",
            "docs": [
              "HashBeast metadata account address"
            ],
            "type": "pubkey"
          },
          {
            "name": "playerMultiplier",
            "docs": [
              "Player's current multiplier after staking"
            ],
            "type": "u16"
          },
          {
            "name": "degenbtcHashpower",
            "docs": [
              "Player's current MINEBTC hashpower after staking"
            ],
            "type": "u64"
          },
          {
            "name": "lpHashpower",
            "docs": [
              "Player's current LP hashpower after staking"
            ],
            "type": "u64"
          },
          {
            "name": "timestamp",
            "docs": [
              "Timestamp of the staking action"
            ],
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "hashBeastSynced",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "hashbeastMetadataAccount",
            "type": "pubkey"
          },
          {
            "name": "dna",
            "type": "bytes"
          },
          {
            "name": "xp",
            "type": "u32"
          },
          {
            "name": "multiplier",
            "type": "u32"
          },
          {
            "name": "accumulatedVal",
            "type": "u64"
          },
          {
            "name": "accumPct",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "hashBeastUnstaked",
      "docs": [
        "Event emitted when a HashBeast is unstaked",
        "Tracks multiplier changes and hashpower updates for indexing"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "docs": [
              "User who unstaked the hashbeast"
            ],
            "type": "pubkey"
          },
          {
            "name": "player",
            "docs": [
              "Player data account address"
            ],
            "type": "pubkey"
          },
          {
            "name": "hashbeastMint",
            "docs": [
              "HashBeast mint address"
            ],
            "type": "pubkey"
          },
          {
            "name": "hashbeastMetadataAccount",
            "docs": [
              "HashBeast metadata account address"
            ],
            "type": "pubkey"
          },
          {
            "name": "playerMultiplier",
            "docs": [
              "Player's current multiplier after unstaking"
            ],
            "type": "u16"
          },
          {
            "name": "degenbtcHashpower",
            "docs": [
              "Player's current MINEBTC hashpower after unstaking"
            ],
            "type": "u64"
          },
          {
            "name": "lpHashpower",
            "docs": [
              "Player's current LP hashpower after unstaking"
            ],
            "type": "u64"
          },
          {
            "name": "timestamp",
            "docs": [
              "Timestamp of the unstaking action"
            ],
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "hashBeastUsedForGameplay",
      "docs": [
        "Event emitted when a HashBeast is used for gameplay"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "hashBeastVisualMutation",
      "docs": [
        "Event emitted when a hashbeast's visual trait is mutated"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "origin",
            "docs": [
              "0 = round claim, 1 = faction-war claim."
            ],
            "type": "u8"
          },
          {
            "name": "originId",
            "type": "u64"
          },
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "traitIndex",
            "type": "u8"
          },
          {
            "name": "oldVal",
            "type": "u8"
          },
          {
            "name": "newVal",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "hashBeastWithdrawnFromGameplay",
      "docs": [
        "Event emitted when a HashBeast is withdrawn from gameplay"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "hashpowerConfig",
      "docs": [
        "------------ HASHPOWER CONFIG ------------",
        "Hashpower configuration for the Minebtc program"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "minLockupDays",
            "docs": [
              "Minimum lockup period in days"
            ],
            "type": "u64"
          },
          {
            "name": "maxLockupDays",
            "docs": [
              "Maximum lockup period in days"
            ],
            "type": "u64"
          },
          {
            "name": "baseMultiplier",
            "docs": [
              "Base multiplier for lockup duration (100 = 1x, separate from BASE_MULTIPLIER=1000 used for hashbeasts)."
            ],
            "type": "u16"
          },
          {
            "name": "maxMultiplier",
            "docs": [
              "Maximum lockup multiplier. Capped at 300 = 3x so total staking boost maxes at 9x with HashBeasts."
            ],
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "hodlPool",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "hodlTaxIndex",
            "type": "u128"
          },
          {
            "name": "totalDbtcClaimable",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "hodlTaxRedistributed",
      "docs": [
        "Event emitted when a degenBTC HODL tax is redistributed through the HODL tax index.",
        "Event emitted when a user pays the HODL tax (\"HODL Tax\") and it gets",
        "redistributed to other users with unclaimed gameplay rewards."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "paperHand",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "taxAmount",
            "type": "u64"
          },
          {
            "name": "redistributedAmount",
            "type": "u64"
          },
          {
            "name": "redistributedIndexIncrement",
            "type": "u128"
          },
          {
            "name": "remainingTotalClaimable",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "inventoryAssetBurned",
      "docs": [
        "An inventory asset was burned because either the trend crashed below",
        "the burn threshold or the entry hit MAX_EXPIRES."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "reason",
            "docs": [
              "0 = trend crash, 1 = max expires, 2 = rebirth queue full"
            ],
            "type": "u8"
          },
          {
            "name": "trendBps",
            "type": "i32"
          },
          {
            "name": "expireCount",
            "type": "u8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "inventoryAssetRelisted",
      "docs": [
        "An inventory asset was relisted at a formula-driven price after either a",
        "fresh sweep or an `expire_program_listing` strike."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "originalBuyPrice",
            "type": "u64"
          },
          {
            "name": "newListPrice",
            "type": "u64"
          },
          {
            "name": "markupBps",
            "type": "i32"
          },
          {
            "name": "trendBps",
            "type": "i32"
          },
          {
            "name": "expireCount",
            "type": "u8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "inventoryPool",
      "docs": [
        "Singleton inventory pool PDA. Seeds: `[b\"inventory-pool\"]`.",
        "",
        "**Dual role:** the PDA at this address simultaneously serves as",
        "1. the typed `Account<InventoryPool>` (this struct — holds counters",
        "and cached marketplace identifiers), and",
        "2. the on-chain custody account: every HashBeast asset the protocol",
        "acquires via `sweep_floor_lowest` or holds for the lootbox queue",
        "has its mpl-core `owner` field set to this PDA.",
        "",
        "Most marketplace ix in `marketplace_cpi.rs` therefore pull this PDA",
        "twice — once as `inventory_pool` (typed view, for counter mutation) and",
        "once as `inventory_pda` (raw view, for asset transfer signer). Same",
        "pubkey, same bump. The PDA signs all asset moves out of inventory",
        "(transfer to user on lootbox claim, list/cancel CPI to the marketplace,",
        "burn CPI to mpl-core) using `[INVENTORY_POOL_SEED, bump]`.",
        "",
        "Sale proceeds: when the marketplace fills one of our inventory listings,",
        "the SOL lands on this PDA as raw lamports above the rent floor.",
        "`handle_inventory_proceeds` routes that surplus 50/50 to",
        "`inventory_sweep_vault` and `sol_treasury`.",
        "",
        "`marketplace_program` / `marketplace_config` are cached at init to avoid",
        "passing them as args; every CPI wrapper validates the caller-supplied",
        "account against the cached pubkey."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "marketplaceProgram",
            "docs": [
              "Cached pubkey of the standalone `degenbtc_market` program. CPI",
              "wrappers `require_keys_eq!(...)` against this."
            ],
            "type": "pubkey"
          },
          {
            "name": "marketplaceConfig",
            "docs": [
              "Cached marketplace `MarketplaceConfig` PDA inside that program.",
              "Also `require_keys_eq!`'d in every wrapper."
            ],
            "type": "pubkey"
          },
          {
            "name": "totalCount",
            "docs": [
              "Live count of NFTs in inventory custody (status: Lootbox or Listed).",
              "Bumped on intake (sweep buy success), decremented on outflow",
              "(`claim_lootbox_nft`, `inventory_finalize_sale`, burn paths in",
              "`expire_program_listing` / `sweep_floor_lowest`). Capped at",
              "`MAX_INVENTORY`. Per-status counts are NOT tracked here — indexers",
              "reconstruct them from `LootboxQueuePush` / `InventoryAssetRelisted`",
              "/ `InventoryAssetBurned` / `InventorySaleFinalized` events."
            ],
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "inventoryPoolInitialized",
      "docs": [
        "One-time emit when the inventory pool is initialized."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "marketplaceProgram",
            "type": "pubkey"
          },
          {
            "name": "marketplaceConfig",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "inventoryProceedsRouted",
      "docs": [
        "`handle_inventory_proceeds` split accumulated inventory SOL into the sweep",
        "reserve and the protocol fee pipeline."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "toSweep",
            "type": "u64"
          },
          {
            "name": "toProtocol",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "inventorySaleFinalized",
      "docs": [
        "Permissionless `inventory_finalize_sale` cleaned up the RebornEntry",
        "after detecting that an inventory listing's asset is no longer owned by",
        "`inventory_pda` (i.e., it sold to a real buyer)."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "keeper",
            "type": "pubkey"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "jackpotHit",
      "docs": [
        "Event emitted by `settle_round` when the global jackpot pot was",
        "successfully paid out to bettors on `faction_id`. Note that `faction_id`",
        "here is the JACKPOT faction (selected by inverse-volume weighting), NOT",
        "the round winner."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "roundId",
            "type": "u64"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "jackpotPotSizeOnHit",
            "docs": [
              "Size of the global jackpot pot at the moment of the hit, snapshotted",
              "onto GameSession.jackpot_pot_size_on_hit before the pot was drained."
            ],
            "type": "u64"
          },
          {
            "name": "jackpotRewardsIndex",
            "docs": [
              "dBTC reward index for any-direction bettors on the jackpot faction",
              "(state.rs:1029). Claim payout = wgtd_points × jackpot_rewards_index / INDEX_PRECISION."
            ],
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "liquidityAdded",
      "docs": [
        "Liquidity added to Raydium pool (before burning LP tokens)"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "solAmount",
            "type": "u64"
          },
          {
            "name": "dbtcAmount",
            "type": "u64"
          },
          {
            "name": "lpTokensMinted",
            "type": "u64"
          },
          {
            "name": "lpTokenPrice",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "liquidityStaked",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "positionIndex",
            "type": "u8"
          },
          {
            "name": "positionKey",
            "type": "pubkey"
          },
          {
            "name": "stakedAmount",
            "type": "u64"
          },
          {
            "name": "weightedAmount",
            "type": "u64"
          },
          {
            "name": "multiplier",
            "type": "u16"
          },
          {
            "name": "lockupDuration",
            "type": "u64"
          },
          {
            "name": "hashpowerContribution",
            "type": "u64"
          },
          {
            "name": "newSolRewards",
            "type": "u64"
          },
          {
            "name": "newDbtcRewards",
            "type": "u64"
          },
          {
            "name": "unrefinedDbtc",
            "type": "u64"
          },
          {
            "name": "newLpSolRewardDebt",
            "type": "u128"
          },
          {
            "name": "newLpDbtcRewardDebt",
            "type": "u128"
          },
          {
            "name": "totalPendingSolRewards",
            "type": "u64"
          },
          {
            "name": "totalPendingStakingDbtcRewards",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "liquidityUnstaked",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "positionIndex",
            "type": "u8"
          },
          {
            "name": "positionKey",
            "type": "pubkey"
          },
          {
            "name": "newSolRewards",
            "type": "u64"
          },
          {
            "name": "newDbtcRewards",
            "type": "u64"
          },
          {
            "name": "unrefinedDbtc",
            "type": "u64"
          },
          {
            "name": "originalAmount",
            "type": "u64"
          },
          {
            "name": "returnedAmount",
            "type": "u64"
          },
          {
            "name": "newLpSolRewardDebt",
            "type": "u128"
          },
          {
            "name": "newLpDbtcRewardDebt",
            "type": "u128"
          },
          {
            "name": "totalPendingSolRewards",
            "type": "u64"
          },
          {
            "name": "totalPendingStakingDbtcRewards",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "lootboxClaim",
      "docs": [
        "Per-user winning-loser reservation. Created lazily inside the claim-rewards",
        "ix only when a loser-roll wins; closed by `claim_lootbox_nft` after a user",
        "or cranker delivers the NFT to the recorded winner.",
        "",
        "`asset == Pubkey::default()` is treated as \"no active reservation\" by",
        "the eligibility check in claim-rewards.",
        "",
        "Seeds: `[b\"lootbox-claim\", user.key().as_ref()]`."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "lootboxNftClaimed",
      "docs": [
        "Reserved hashbeast was delivered to the recorded user. `cranker` is the signer",
        "that paid the delivery transaction; it may be the user or a bot."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "cranker",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "rebirthCount",
            "type": "u8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "lootboxQueue",
      "docs": [
        "Per-country lootbox queue. One PDA per faction. Rebirth and sweep-buy",
        "flows push assets into `slots[..filled_count]` (always packed). Loser-roll",
        "pops a random index out and shifts left.",
        "",
        "Seeds: `[b\"lootbox-queue\", &[faction_id]]`."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "slots",
            "docs": [
              "Packed asset addresses. `slots[..filled_count]` is the live window."
            ],
            "type": {
              "array": [
                "pubkey",
                10
              ]
            }
          },
          {
            "name": "filledCount",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "lootboxQueueInitialized",
      "docs": [
        "One-time emit when a country's lootbox queue PDA is created at admin setup."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "queuePda",
            "type": "pubkey"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "lootboxQueuePush",
      "docs": [
        "An asset was pushed into a country lootbox queue (from `rebirth_hashbeast`,",
        "`sweep_floor_lowest`, or `expire_program_listing`). `queue_depth_after`",
        "reflects post-push state."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "queueDepthAfter",
            "type": "u8"
          },
          {
            "name": "source",
            "type": "u8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "lootboxRollMissed",
      "docs": [
        "A losing player's claim ix triggered a roll that MISSED. Queue unchanged."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "queueDepth",
            "type": "u8"
          },
          {
            "name": "rollValue",
            "type": "u16"
          },
          {
            "name": "thresholdBps",
            "type": "u16"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "lootboxRollWon",
      "docs": [
        "A losing player's claim ix triggered a roll that WON. Asset is reserved",
        "for them via `LootboxClaim` PDA until a user or cranker delivers it with",
        "`claim_lootbox_nft`."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "queueDepthBefore",
            "type": "u8"
          },
          {
            "name": "rollValue",
            "type": "u16"
          },
          {
            "name": "thresholdBps",
            "type": "u16"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "lpStakingRewardsDistributed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "roundId",
            "type": "u64"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "totalLpHashpower",
            "docs": [
              "Hashpower denominator used to compute the emitted reward indexes."
            ],
            "type": "u64"
          },
          {
            "name": "dbtcStakerRewards",
            "type": "u64"
          },
          {
            "name": "solStakerRewards",
            "type": "u64"
          },
          {
            "name": "lpDegenbtcRewardIndex",
            "type": "u128"
          },
          {
            "name": "lpSolRewardIndex",
            "type": "u128"
          }
        ]
      }
    },
    {
      "name": "lpTokensBurned",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "lpTokensBurned",
            "type": "u64"
          },
          {
            "name": "totalLpBurnt",
            "type": "u64"
          },
          {
            "name": "dbtcAmountAdded",
            "type": "u64"
          },
          {
            "name": "solAmountAdded",
            "type": "u64"
          },
          {
            "name": "lpTokenPrice",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "mineBtcStaked",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "positionIndex",
            "type": "u8"
          },
          {
            "name": "positionKey",
            "type": "pubkey"
          },
          {
            "name": "stakedAmount",
            "type": "u64"
          },
          {
            "name": "weightedAmount",
            "type": "u64"
          },
          {
            "name": "multiplier",
            "type": "u16"
          },
          {
            "name": "lockupDuration",
            "type": "u64"
          },
          {
            "name": "hashpowerContribution",
            "type": "u64"
          },
          {
            "name": "newSolRewards",
            "type": "u64"
          },
          {
            "name": "newDbtcRewards",
            "type": "u64"
          },
          {
            "name": "unrefinedDbtc",
            "type": "u64"
          },
          {
            "name": "newDegenbtcSolRewardDebt",
            "type": "u128"
          },
          {
            "name": "newDegenbtcDbtcRewardDebt",
            "type": "u128"
          },
          {
            "name": "totalPendingSolRewards",
            "type": "u64"
          },
          {
            "name": "totalPendingStakingDbtcRewards",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "mineBtcUnstaked",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "positionIndex",
            "type": "u8"
          },
          {
            "name": "positionKey",
            "type": "pubkey"
          },
          {
            "name": "newSolRewards",
            "type": "u64"
          },
          {
            "name": "newDbtcRewards",
            "type": "u64"
          },
          {
            "name": "unrefinedDbtc",
            "type": "u64"
          },
          {
            "name": "originalAmount",
            "type": "u64"
          },
          {
            "name": "returnedAmount",
            "type": "u64"
          },
          {
            "name": "newDegenbtcSolRewardDebt",
            "type": "u128"
          },
          {
            "name": "newDegenbtcDbtcRewardDebt",
            "type": "u128"
          },
          {
            "name": "totalPendingSolRewards",
            "type": "u64"
          },
          {
            "name": "totalPendingStakingDbtcRewards",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "minebtcClaimableAccrued",
      "docs": [
        "Event emitted whenever pending degenBTC claimable balance is increased.",
        "`source_amount` is the new reward from the triggering action, while",
        "`unrefined_bonus_amount` is previously deferred hodl-tax yield realized at the same time."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "source",
            "type": "u8"
          },
          {
            "name": "referenceId",
            "type": "u64"
          },
          {
            "name": "sourceAmount",
            "type": "u64"
          },
          {
            "name": "unrefinedBonusAmount",
            "type": "u64"
          },
          {
            "name": "totalAdded",
            "type": "u64"
          },
          {
            "name": "pendingDbtcAfter",
            "type": "u64"
          },
          {
            "name": "totalClaimableAfter",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "miningTokenVaultSet",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "authority",
            "docs": [
              "The authority that set the token vault"
            ],
            "type": "pubkey"
          },
          {
            "name": "tokenVault",
            "docs": [
              "The token vault address"
            ],
            "type": "pubkey"
          },
          {
            "name": "tokenVaultAuthority",
            "docs": [
              "The token vault authority address"
            ],
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "nftMarketMakingFunded",
      "docs": [
        "`distribute_sol_fees` peeled off `nft_market_making_pct` of available SOL",
        "and routed it directly to `inventory_sweep_vault` to fund permissionless",
        "NFT market-making (sweep buys + keeper bounties). Replaces the old",
        "dbtc-tax → Raydium swap → SOL refill flow."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "solAmount",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "paperHandBurned",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "positionIndex",
            "type": "u8"
          },
          {
            "name": "positionKey",
            "type": "pubkey"
          },
          {
            "name": "stakedTokenType",
            "type": "u8"
          },
          {
            "name": "originalAmount",
            "type": "u64"
          },
          {
            "name": "penaltyAmount",
            "type": "u64"
          },
          {
            "name": "returnedAmount",
            "type": "u64"
          },
          {
            "name": "penaltyTaxPct",
            "type": "u64"
          },
          {
            "name": "daysRemaining",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "playerData",
      "docs": [
        "Player Data PDA (Seed: `[b\"player\", user_pubkey]`)",
        "Persistent account for each player that tracks:",
        "- Player statistics (rounds played, won, total bets/winnings)",
        "- List of rounds the player participated in (for tracking unclaimed rewards)",
        "- Passive staking data (hashpower, reward indexes)",
        "Each user bet in a round has its own UserGameBet PDA, referenced here via round IDs."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "owner",
            "docs": [
              "The user's wallet address"
            ],
            "type": "pubkey"
          },
          {
            "name": "referralCode",
            "docs": [
              "Referral code used by this player"
            ],
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "docs": [
              "The faction this player is assigned to"
            ],
            "type": "u8"
          },
          {
            "name": "originFactionId",
            "docs": [
              "Permanent faction chosen at signup. Country identity does not change after registration."
            ],
            "type": "u8"
          },
          {
            "name": "referrerFactionId",
            "docs": [
              "Referrer's origin faction at signup, or u8::MAX when there is no referrer."
            ],
            "type": "u8"
          },
          {
            "name": "degenbtcHashpower",
            "type": "u64"
          },
          {
            "name": "degenbtcStaked",
            "type": "u64"
          },
          {
            "name": "degenbtcDegenbtcRewardDebt",
            "type": "u128"
          },
          {
            "name": "degenbtcSolRewardDebt",
            "type": "u128"
          },
          {
            "name": "lpHashpower",
            "type": "u64"
          },
          {
            "name": "lpStaked",
            "type": "u64"
          },
          {
            "name": "lpSolRewardDebt",
            "type": "u128"
          },
          {
            "name": "lpDegenbtcRewardDebt",
            "type": "u128"
          },
          {
            "name": "pendingSolRewards",
            "type": "u64"
          },
          {
            "name": "hodlTaxIndex",
            "type": "u128"
          },
          {
            "name": "pendingDbtcRewards",
            "docs": [
              "Gameplay-earned degenBTC rewards pending HODL-tax withdrawal."
            ],
            "type": "u64"
          },
          {
            "name": "pendingStakingDbtcRewards",
            "docs": [
              "Passive staking degenBTC rewards pending direct claim with SOL staking rewards."
            ],
            "type": "u64"
          },
          {
            "name": "unrefinedDbtcRewards",
            "type": "u64"
          },
          {
            "name": "pendingRoundClaims",
            "docs": [
              "Number of unclaimed per-round reward accounts still outstanding."
            ],
            "type": "u16"
          },
          {
            "name": "pendingWarClaims",
            "docs": [
              "Number of unclaimed per-faction-war reward accounts still outstanding."
            ],
            "type": "u16"
          },
          {
            "name": "degenbtcPositionIndices",
            "type": "bytes"
          },
          {
            "name": "lpPositionIndices",
            "type": "bytes"
          },
          {
            "name": "stakedHashbeasts",
            "docs": [
              "Staked dragon hashbeasts (max 3 hashbeasts)",
              "Stores the mint addresses of staked hashbeasts"
            ],
            "type": {
              "vec": "pubkey"
            }
          },
          {
            "name": "hashbeastMultiplier",
            "docs": [
              "Current hashbeast multiplier (1000 = 1x, 1500 = 1.5x, etc.)",
              "Effective passive staking HashBeast multiplier after applying the 3x passive cap."
            ],
            "type": "u16"
          },
          {
            "name": "freeTickets",
            "docs": [
              "Free tickets: points size of each ticket type (max 5 ticket types)",
              "Example: [10000000, 100000000, ...] where 1 point = 1 SOL lamport",
              "So 10000000 = 0.01 SOL, 100000000 = 0.1 SOL"
            ],
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "freeTicketsRemaining",
            "docs": [
              "Free tickets remaining: count of each ticket type remaining",
              "Index matches free_tickets (e.g., free_tickets_remaining[0] is count for free_tickets[0])"
            ],
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "gameplayHashbeast",
            "docs": [
              "HashBeast currently being used in gameplay (Pubkey::default() if none)"
            ],
            "type": "pubkey"
          },
          {
            "name": "activeMultiplier",
            "docs": [
              "Active gameplay multiplier (1000 = 1x, set from gameplay hashbeast's multiplier, capped at 4.2x, reset to BASE_MULTIPLIER on withdraw)"
            ],
            "type": "u32"
          },
          {
            "name": "gameplayHashbeastDna",
            "docs": [
              "Cached DNA of gameplay hashbeast (for mutation calculations without loading HashBeastMetadata)"
            ],
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "gameplayHashbeastXp",
            "docs": [
              "Cached XP of gameplay hashbeast (updated during gameplay, synced to HashBeastMetadata on withdraw)"
            ],
            "type": "u32"
          },
          {
            "name": "gameplayUnlockRequestFactionWar",
            "docs": [
              "FactionWar ID in which the user requested gameplay unlock.",
              "The hashbeast can only be withdrawn once the next faction_war cycle begins."
            ],
            "type": "u64"
          },
          {
            "name": "currentWarScore",
            "docs": [
              "Cumulative gameplay score for the current faction war cycle.",
              "Lazy-reset to 0 the first time it's touched in a new cycle (see",
              "`current_war_score_cycle_id` below). Used for MVP tracking."
            ],
            "type": "u64"
          },
          {
            "name": "currentWarScoreCycleId",
            "docs": [
              "`war_id` that `current_war_score` belongs to.",
              "On the first bet of a new cycle (when `war_state.war_id`",
              "differs from this), the running score is reset to 0 and this is updated.",
              "This avoids needing a separate per-user reset instruction at cycle rollover."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "playerInitialized",
      "docs": [
        "Event emitted when a player initializes their account"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "originFactionId",
            "type": "u8"
          },
          {
            "name": "referralCode",
            "type": {
              "option": "pubkey"
            }
          },
          {
            "name": "referrerFactionId",
            "type": {
              "option": "u8"
            }
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "playerRecruited",
      "docs": [
        "Event emitted when a player joins through the country referral loop."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "player",
            "type": "pubkey"
          },
          {
            "name": "referrer",
            "type": "pubkey"
          },
          {
            "name": "playerOriginFactionId",
            "type": "u8"
          },
          {
            "name": "referrerOriginFactionId",
            "type": "u8"
          },
          {
            "name": "referrerTotalRecruits",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "predictionDirection",
      "docs": [
        "Directional stance for country bets (rounds + cycle leaderboard)."
      ],
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "down"
          },
          {
            "name": "neutral"
          },
          {
            "name": "up"
          }
        ]
      }
    },
    {
      "name": "priceEntry",
      "docs": [
        "------------ HASHBEAST-BTC MINING ------------",
        "Price entry for tracking historical prices"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "timestamp",
            "docs": [
              "Timestamp when this price was recorded"
            ],
            "type": "i64"
          },
          {
            "name": "price",
            "docs": [
              "Price in SOL per MINE_BTC (scaled by 10^9 for full precision)",
              "This matches SOL's decimal precision for accurate price tracking"
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "priceSnapshotTaken",
      "docs": [
        "Price snapshot taken every 30 minutes (1-8 snapshots per 4-hour cycle)"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "snapshotNumber",
            "type": "u8"
          },
          {
            "name": "solSwapped",
            "type": "u64"
          },
          {
            "name": "dbtcReceived",
            "type": "u64"
          },
          {
            "name": "currentPrice",
            "type": "u64"
          },
          {
            "name": "weightedAvgPrice",
            "type": "u64"
          },
          {
            "name": "solEarmarkedForPol",
            "type": "u64"
          },
          {
            "name": "totalPolBalance",
            "type": "u64"
          },
          {
            "name": "priceHistoryCount",
            "type": "u8"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "programListingExpired",
      "docs": [
        "An inventory listing that sat unsold for `EXPIRE_GRACE_SECS` was expired.",
        "Disposition cascade follows in a separate event."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "previousListPrice",
            "type": "u64"
          },
          {
            "name": "expireCountAfter",
            "type": "u8"
          },
          {
            "name": "keeper",
            "type": "pubkey"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "protocolOwnedLiquidity",
      "docs": [
        "Protocol Owned Liquidity tracking for comprehensive POL metrics"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "totalLpBurnt",
            "docs": [
              "Total LP tokens burned (accumulated)"
            ],
            "type": "u64"
          },
          {
            "name": "lpOperationsCount",
            "docs": [
              "Number of LP addition operations performed"
            ],
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "rebornEntry",
      "docs": [
        "One per HashBeast currently held by `inventory_pda`.",
        "Seeds: `[b\"reborn-entry\", asset]`.",
        "",
        "**Lifecycle:**",
        "",
        "```text",
        "┌───────────────────────────────────────────────────────────────────┐",
        "│ INTAKE (creates RebornEntry, +1 InventoryPool.total_count)        │",
        "│   sweep_floor_lowest → status = Lootbox  OR  Listed  OR  (burn,   │",
        "│                        no entry)                                   │",
        "│   rebirth_hashbeast   → status = Lootbox (no relist path)         │",
        "└───────────────────────────────────────────────────────────────────┘",
        "│",
        "▼",
        "┌───────────────────────────────────────────────────────────────────┐",
        "│ ACTIVE                                                             │",
        "│   Lootbox: sits in `LootboxQueue[faction_id]`; awaits loser-roll. │",
        "│   Listed:  live program-owned listing on marketplace.             │",
        "└───────────────────────────────────────────────────────────────────┘",
        "│",
        "┌─────────────────┼──────────────────┐",
        "▼                 ▼                  ▼",
        "┌────────────────────┐ ┌────────────────┐ ┌──────────────────────┐",
        "│ claim_lootbox_nft  │ │ inventory_     │ │ expire_program_      │",
        "│ (Lootbox → user)   │ │ finalize_sale  │ │ listing              │",
        "│ closes RebornEntry │ │ (Listed sold)  │ │ (Listed unsold @ 7d) │",
        "│ -1 total_count     │ │ closes entry   │ │ cancel + cascade:    │",
        "└────────────────────┘ │ -1 total_count │ │  - relist (++strike) │",
        "└────────────────┘ │  - lootbox push      │",
        "│  - burn @ MAX_EXPIRES│",
        "└──────────────────────┘",
        "```",
        "",
        "**`original_buy_price`** is the immutable anchor for relist markup math.",
        "Across multiple expire/relist cycles, each new list price is computed as",
        "`apply_markup(original_buy_price, markup_bps)` where `markup_bps` depends",
        "on the floor trend and `expire_count`. This keeps the protocol's effective",
        "resale \"cost basis\" stable even as the asset is repriced over time.",
        "",
        "**Quality score** is fixed at intake from",
        "`compute_quality_score(multiplier, xp, breed_count)`. Indexers use it for",
        "\"rare drop\" UX."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "qualityScore",
            "docs": [
              "0..=10_000. Snapshot at intake; never updated."
            ],
            "type": "u16"
          },
          {
            "name": "rebornAt",
            "type": "i64"
          },
          {
            "name": "status",
            "docs": [
              "`RebornStatus` enum value (Lootbox | Listed)."
            ],
            "type": "u8"
          },
          {
            "name": "listingPrice",
            "docs": [
              "Current live listing price (lamports); 0 if status != Listed."
            ],
            "type": "u64"
          },
          {
            "name": "origin",
            "docs": [
              "`RebornOrigin` enum value (Reborn | Swept)."
            ],
            "type": "u8"
          },
          {
            "name": "originalBuyPrice",
            "docs": [
              "Immutable cost basis: the price the protocol paid for the asset",
              "(sweep buy amount, or 0 for rebirth-origin entries). Used as the",
              "base of the relist markup formula across expire cycles, so the",
              "protocol's effective floor for resale doesn't drift downward as",
              "strikes accumulate."
            ],
            "type": "u64"
          },
          {
            "name": "expireCount",
            "docs": [
              "Number of times `expire_program_listing` has fired for this entry.",
              "Each strike subtracts `RELIST_EXPIRE_PENALTY_BPS` from the markup",
              "formula. Forced burn at `MAX_EXPIRES`."
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "referralRewards",
      "docs": [
        "Stores referral rewards that a user has earned from referrals.",
        "Rewards accrue as a slice of SOL protocol fees the referee actually pays",
        "(bets + NFT mints), capped at MAX_REFERRER_SOL_LIFETIME. After the cap,",
        "new fees flow back to the normal recipients (no further accrual)."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "type": "pubkey"
          },
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "ownerFactionId",
            "docs": [
              "Permanent faction of the referral-code owner."
            ],
            "type": "u8"
          },
          {
            "name": "referralsCount",
            "docs": [
              "Number of users who have used this user's referral code.",
              "This is analytics/accounting only; registration is not capped by count."
            ],
            "type": "u64"
          },
          {
            "name": "pendingSolRewards",
            "docs": [
              "Pending SOL rewards from referees' protocol fees (bets + NFT mints).",
              "Stored as extra lamports on this PDA; claimed via claim_referral_rewards."
            ],
            "type": "u64"
          },
          {
            "name": "totalSolEarned",
            "docs": [
              "Cumulative SOL earned across all referees. Capped at MAX_REFERRER_SOL_LIFETIME;",
              "once total_sol_earned >= cap, no further accrual occurs."
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "referralRewardsClaimed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "referrer",
            "type": "pubkey"
          },
          {
            "name": "referralRewardsAccount",
            "type": "pubkey"
          },
          {
            "name": "dbtcAmount",
            "type": "u64"
          },
          {
            "name": "solAmount",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "rewardsDistributedForRound",
      "docs": [
        "Event emitted by `settle_round` after `track_war_round_completion`",
        "runs. Carries the drought-volume snapshot that fed into the mutation roll for",
        "this round's claimers (state.rs:1048-1053, used at user.rs:1689)."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "roundId",
            "type": "u64"
          },
          {
            "name": "winningFactionId",
            "type": "u8"
          },
          {
            "name": "winningDirection",
            "type": "u8"
          },
          {
            "name": "solRewardsIndex",
            "docs": [
              "Final exact-winner SOL index after settle_round redirects any orphaned",
              "staker SOL fees back to exact winners."
            ],
            "type": "u128"
          },
          {
            "name": "dbtcRewardsIndex",
            "docs": [
              "Final exact-winner degenBTC index after settle_round redirects any",
              "orphaned staker degenBTC back to exact winners."
            ],
            "type": "u128"
          },
          {
            "name": "winningFactionVolumeAtRound",
            "docs": [
              "Frozen value of the winning faction's `sol_volume_since_last_win` at",
              "round-end, BEFORE the counter was reset to 0. Late claims hours later",
              "will still see this same number when computing volume_factor."
            ],
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "roundEnded",
      "docs": [
        "Event emitted when a round ends (after winner selection and reward calculations).",
        "",
        "Carries everything the off-chain indexer needs to render `latest_result`",
        "WITHOUT having to fetch the `GameSession` PDA. All these fields are already",
        "populated on the PDA by the time `emit_round_ended` is called (game.rs:559).",
        "`winning_faction_volume_at_round` is the one exception — it lands later in",
        "`track_war_round_completion` and ships on `RewardsDistributedForRound`."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "roundId",
            "type": "u64"
          },
          {
            "name": "gameSession",
            "type": "pubkey"
          },
          {
            "name": "winningFactionId",
            "type": "u8"
          },
          {
            "name": "winningDirection",
            "type": "u8"
          },
          {
            "name": "entropySlotUsed",
            "type": "u64"
          },
          {
            "name": "usedEntropyFallback",
            "type": "bool"
          },
          {
            "name": "totalSolBets",
            "type": "u64"
          },
          {
            "name": "totalPointsBets",
            "type": "u64"
          },
          {
            "name": "totalWgtdPointsBets",
            "type": "u64"
          },
          {
            "name": "userBetsCount",
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "factionSolBets",
            "type": {
              "array": [
                "u64",
                12
              ]
            }
          },
          {
            "name": "dbtcWinnerPool",
            "type": "u64"
          },
          {
            "name": "dbtcSameFactionDirectionPools",
            "type": {
              "array": [
                "u64",
                3
              ]
            }
          },
          {
            "name": "dbtcStakers",
            "type": "u64"
          },
          {
            "name": "dbtcJackpot",
            "type": "u64"
          },
          {
            "name": "jackpotHit",
            "type": "bool"
          },
          {
            "name": "jackpotFactionId",
            "type": "u8"
          },
          {
            "name": "stakersFee",
            "docs": [
              "Σ stakers_fee_per_bet accumulated by internal_process_bets (user.rs:2544).",
              "Indexer reverses this to derive effective_fee = stakers_fee × 100 / stakers_pct,",
              "then splits the residual into buybacks / nft_mm / dev_fee per economy.rs."
            ],
            "type": "u64"
          },
          {
            "name": "solRewardsIndex",
            "docs": [
              "Reward indexes finalized inside end_round. SOL paid by raw points,",
              "dBTC paid by weighted points (hashbeast multiplier applies to dBTC only)."
            ],
            "type": "u128"
          },
          {
            "name": "dbtcRewardsIndex",
            "type": "u128"
          },
          {
            "name": "mutationsPerFaction",
            "docs": [
              "Mutation tally for this round (state.rs:1037-1040)."
            ],
            "type": {
              "array": [
                "u8",
                12
              ]
            }
          },
          {
            "name": "totalMutationsThisRound",
            "type": "u8"
          },
          {
            "name": "warIdWhenPlayed",
            "docs": [
              "Cycle ID snapshot at round-start, frozen onto the GameSession so late",
              "claims hit the right FactionWarState PDA even after the cycle settles",
              "(state.rs:1042-1046)."
            ],
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "roundRewardsClaimed",
      "docs": [
        "Event emitted when a user claims rewards for a round"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "roundId",
            "type": "u64"
          },
          {
            "name": "solReward",
            "type": "u64"
          },
          {
            "name": "dbtcReward",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "roundStarted",
      "docs": [
        "Event emitted when a new round starts"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "roundId",
            "type": "u64"
          },
          {
            "name": "gameSession",
            "type": "pubkey"
          },
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "roundStartSlot",
            "type": "u64"
          },
          {
            "name": "roundStartTimestamp",
            "type": "i64"
          },
          {
            "name": "roundEndTimestamp",
            "type": "i64"
          },
          {
            "name": "scheduledEntropySlot",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "solFeeConfig",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "protocolFeePct",
            "docs": [
              "Whole-percent share of SOL fees that goes to protocol. `100` = 100%."
            ],
            "type": "u8"
          },
          {
            "name": "buybackPct",
            "docs": [
              "Whole-percent share of SOL fees that goes to buybacks. `100` = 100%."
            ],
            "type": "u8"
          },
          {
            "name": "stakersPct",
            "docs": [
              "Whole-percent share of SOL fees that goes to stakers. `100` = 100%."
            ],
            "type": "u8"
          },
          {
            "name": "cycleSolSplitPct",
            "docs": [
              "Whole-percent share of the user's SOL bet reserved for the faction-war",
              "cycle SOL reward pool. Taken directly from the gross bet, in addition to the",
              "protocol fee. `100` = 100%."
            ],
            "type": "u8"
          },
          {
            "name": "nftMarketMakingPct",
            "docs": [
              "Whole-percent share of `distribute_sol_fees` SOL routed to the",
              "`inventory_sweep_vault` PDA, funding permissionless NFT market making",
              "(sweep buys + keeper bounties). `100` = 100%."
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "solFeesWithdrawn",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "availableSolana",
            "type": "u64"
          },
          {
            "name": "buybackAmount",
            "type": "u64"
          },
          {
            "name": "nftMarketMakingAmount",
            "type": "u64"
          },
          {
            "name": "devEarningsAmount",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "solRewardsClaimed",
      "docs": [
        "Event emitted when a user claims passive staking rewards."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "playerData",
            "type": "pubkey"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "solAmount",
            "type": "u64"
          },
          {
            "name": "dbtcAmount",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "stakedPosition",
      "docs": [
        "Individual degenBTC staking position"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "positionType",
            "type": "u8"
          },
          {
            "name": "positionIndex",
            "type": "u8"
          },
          {
            "name": "factionId",
            "type": "u8"
          },
          {
            "name": "stakedAmount",
            "docs": [
              "Staking details"
            ],
            "type": "u64"
          },
          {
            "name": "weightedAmount",
            "type": "u64"
          },
          {
            "name": "startTimestamp",
            "type": "i64"
          },
          {
            "name": "lockupEndTimestamp",
            "type": "i64"
          },
          {
            "name": "lockupDuration",
            "type": "u64"
          },
          {
            "name": "multiplier",
            "type": "u16"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "storyEventTriggered",
      "docs": [
        "Event emitted when gameplay creates a story-worthy HashBeast event.",
        "",
        "The contract may still mutate DNA / XP / multiplier as part of the event,",
        "but off-chain systems should treat this as a flexible story hook. A backend",
        "can turn it into artwork, reels, character history, or a simple indexed beat."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "origin",
            "docs": [
              "0 = round claim, 1 = faction-war claim."
            ],
            "type": "u8"
          },
          {
            "name": "originId",
            "type": "u64"
          },
          {
            "name": "user",
            "type": "pubkey"
          },
          {
            "name": "hashbeastMint",
            "type": "pubkey"
          },
          {
            "name": "storyEventType",
            "type": "u8"
          },
          {
            "name": "xpGained",
            "type": "u32"
          },
          {
            "name": "multiplierAfter",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "taxConfig",
      "docs": [
        "Tax Configuration PDA (Seed: `[b\"tax-config\"]`)",
        "Manages degenBTC transfer-tax distribution: faction treasury + burn + the",
        "residual flowing back to the mining vault. NFT market-making is funded",
        "from SOL (see `SolFeeConfig::nft_market_making_pct`), not from this tax."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "treasuryPct",
            "docs": [
              "Percentage of withheld tax that goes to faction treasury"
            ],
            "type": "u8"
          },
          {
            "name": "burnPct",
            "docs": [
              "Percentage of withheld tax that gets burned (remainder goes back to vault)"
            ],
            "type": "u8"
          },
          {
            "name": "totalBurnt",
            "docs": [
              "Total amount of degenBTC burnt so far (cumulative)"
            ],
            "type": "u64"
          },
          {
            "name": "unassignedWarTreasuryAmount",
            "docs": [
              "Treasury tax accrued while no active faction war state existed yet.",
              "This amount gets attached to the next faction war when that state is initialized."
            ],
            "type": "u64"
          },
          {
            "name": "withdrawWithheldAuthority",
            "docs": [
              "PDA addresses for tax system"
            ],
            "type": "pubkey"
          },
          {
            "name": "factionTreasuryVault",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "taxDistributed",
      "docs": [
        "Event emitted when tax is distributed from mint to vaults"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "totalTaxAmount",
            "type": "u64"
          },
          {
            "name": "factionTreasuryAmount",
            "docs": [
              "Pre-fee amount transferred toward the faction treasury vault."
            ],
            "type": "u64"
          },
          {
            "name": "factionTreasuryCredit",
            "docs": [
              "Post-fee amount actually delivered and credited to a faction war."
            ],
            "type": "u64"
          },
          {
            "name": "burnAmount",
            "type": "u64"
          },
          {
            "name": "vaultReturnAmount",
            "docs": [
              "Amount returned to the degenBTC emission vault."
            ],
            "type": "u64"
          },
          {
            "name": "totalBurnt",
            "type": "u64"
          },
          {
            "name": "warId",
            "type": "u64"
          },
          {
            "name": "creditedToActiveWar",
            "type": "bool"
          },
          {
            "name": "unassignedWarTreasuryAmount",
            "type": "u64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "ticketTier",
      "docs": [
        "Ticket tier option for hashbeast minting",
        "When users mint hashbeasts, they choose a ticket tier which gives them free tickets"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "ticketValue",
            "docs": [
              "Ticket value in lamports (e.g., 10_000_000 = 0.01 SOL)"
            ],
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "userFactionWarBets",
      "docs": [
        "User FactionWar Bets PDA (Seed: `[b\"user-faction-war\", user_pubkey, war_id_u64_le]`)",
        "Tracks how much weighted stake a user bet on each faction's direction during a",
        "specific faction_war. These weights power the global base cycle rewards."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "owner",
            "docs": [
              "The user who placed these bets"
            ],
            "type": "pubkey"
          },
          {
            "name": "warId",
            "docs": [
              "The faction-war ID this tracks"
            ],
            "type": "u64"
          },
          {
            "name": "gameplayHashbeast",
            "docs": [
              "Gameplay hashbeast that backed home country during the faction_war.",
              "Set on the user's first home-faction bet while an HB is deployed;",
              "validated to stay the same for subsequent home bets in the cycle."
            ],
            "type": "pubkey"
          },
          {
            "name": "mutationScore",
            "docs": [
              "Cumulative mutation-bonus score this user contributed to their home",
              "country during the cycle. Incremented in `apply_mutation_bonus_score`",
              "on each successful round-claim mutation roll. Used as the HB-bonus",
              "numerator at war claim (`hb_share = pool * mutation_score / faction_mutation_score`)."
            ],
            "type": "u64"
          },
          {
            "name": "directionBets",
            "docs": [
              "Weighted bet per faction and direction during this faction_war."
            ],
            "type": {
              "array": [
                {
                  "array": [
                    "u64",
                    3
                  ]
                },
                12
              ]
            }
          },
          {
            "name": "solDirectionBets",
            "docs": [
              "Real SOL bet per faction and direction during this faction_war. Ticket bets stay zero here."
            ],
            "type": {
              "array": [
                {
                  "array": [
                    "u64",
                    3
                  ]
                },
                12
              ]
            }
          }
        ]
      }
    },
    {
      "name": "userGameBet",
      "docs": [
        "User Game Bet PDA (Seed: `[b\"user-bet\", user_pubkey, round_id_u64]`)",
        "Each user bet in a round has its own PDA account.",
        "Users can bet on multiple faction-direction positions in a single round,",
        "including multiple directions on the same faction.",
        "",
        "Structure:",
        "- `faction_ids`: List of factions user bet on",
        "- `directions`: Direction chosen for each faction (0=Down, 1=Neutral, 2=Up)",
        "- `sol_bets`: SOL bets for each faction (index matches faction_ids)",
        "- `points_bets`: Points bets for each faction (index matches faction_ids)",
        "- `total_sol_bet`: Total SOL bet across all factions",
        "- `total_points_bet`: Total points bet across all factions",
        "- `total_fee`: Total fees paid"
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "owner",
            "docs": [
              "The user who placed this bet"
            ],
            "type": "pubkey"
          },
          {
            "name": "roundId",
            "docs": [
              "The round ID this bet belongs to"
            ],
            "type": "u64"
          },
          {
            "name": "warId",
            "docs": [
              "Faction-war cycle active when this round bet was placed."
            ],
            "type": "u64"
          },
          {
            "name": "factionIds",
            "docs": [
              "List of faction IDs user bet on.",
              "Index position corresponds to the same index in directions/sol_bets/points_bets."
            ],
            "type": "bytes"
          },
          {
            "name": "directions",
            "docs": [
              "Direction chosen for each faction (0=Down, 1=Neutral, 2=Up)."
            ],
            "type": "bytes"
          },
          {
            "name": "solBets",
            "docs": [
              "SOL bets for each faction (index matches faction_ids)"
            ],
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "pointsBets",
            "docs": [
              "Points bets for each faction (index matches faction_ids)"
            ],
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "wgtdPointsBets",
            "docs": [
              "Weighted points for each faction (points * multiplier / 100 for SOL, else points) - for degenBTC"
            ],
            "type": {
              "vec": "u64"
            }
          },
          {
            "name": "totalSolBet",
            "docs": [
              "Total SOL amount bet across all factions (after protocol fee deduction)"
            ],
            "type": "u64"
          },
          {
            "name": "totalPointsBet",
            "docs": [
              "Total points amount bet across all factions"
            ],
            "type": "u64"
          },
          {
            "name": "totalWgtdPointsBet",
            "docs": [
              "Total weighted points (for degenBTC rewards)"
            ],
            "type": "u64"
          },
          {
            "name": "totalFee",
            "docs": [
              "Total fees paid across all bets"
            ],
            "type": "u64"
          },
          {
            "name": "gameplayHashbeast",
            "type": "pubkey"
          },
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "mutationType",
            "docs": [
              "0 = no mutation, 1 = Evolution, 2 = Power, 3 = Trait"
            ],
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "userSaleRecorded",
      "docs": [
        "A user-to-user marketplace sale qualified as a real-demand snapshot input."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "buyer",
            "type": "pubkey"
          },
          {
            "name": "seller",
            "type": "pubkey"
          },
          {
            "name": "price",
            "type": "u64"
          },
          {
            "name": "listingAgeSecs",
            "type": "i64"
          },
          {
            "name": "timestamp",
            "type": "i64"
          }
        ]
      }
    }
  ]
};
