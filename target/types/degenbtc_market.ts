/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/degenbtc_market.json`.
 */
export type DegenbtcMarket = {
  "address": "Dh4LLRQHqmBeYHYXs3haYTsXTkBa11APJqq6korkc4hg",
  "metadata": {
    "name": "degenbtcMarket",
    "version": "0.1.0",
    "spec": "0.1.0",
    "description": "MineBTC degenBTC marketplace — minimal mpl-core NFT marketplace for the HashBeast collection"
  },
  "instructions": [
    {
      "name": "buyListing",
      "docs": [
        "Buyer pays SOL (fee + proceeds), asset hops escrow → buyer, listing closes."
      ],
      "discriminator": [
        115,
        149,
        42,
        108,
        44,
        49,
        140,
        153
      ],
      "accounts": [
        {
          "name": "payer",
          "docs": [
            "Pays SOL (price) and any mpl-core transfer reallocations. Usually the",
            "same wallet as `buyer`; protocol sweeps may use a separate system-owned",
            "payer PDA while sending the NFT to inventory."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "buyer",
          "docs": [
            "Receives the asset. Does not need to sign."
          ],
          "writable": true
        },
        {
          "name": "seller",
          "docs": [
            "Receives `price - fee` in SOL plus the listing rent refund."
          ],
          "writable": true
        },
        {
          "name": "marketplaceConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  112,
                  108,
                  97,
                  99,
                  101,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplace_config.collection_mint",
                "account": "marketplaceConfig"
              }
            ]
          }
        },
        {
          "name": "listing",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  115,
                  116,
                  105,
                  110,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "asset"
              }
            ]
          }
        },
        {
          "name": "asset",
          "writable": true,
          "relations": [
            "listing"
          ]
        },
        {
          "name": "collection",
          "writable": true
        },
        {
          "name": "escrow",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  115,
                  99,
                  114,
                  111,
                  119
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "asset"
              }
            ]
          }
        },
        {
          "name": "feeRecipient",
          "writable": true
        },
        {
          "name": "mplCoreProgram"
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
      "name": "cancelListing",
      "docs": [
        "Seller cancels — asset returns to seller, listing closes (rent refund)."
      ],
      "discriminator": [
        41,
        183,
        50,
        232,
        230,
        233,
        157,
        70
      ],
      "accounts": [
        {
          "name": "payer",
          "docs": [
            "Pays any mpl-core transfer reallocations. Usually the same wallet as",
            "`seller`; protocol-owned listings may use a separate system-owned",
            "payer PDA."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "seller",
          "docs": [
            "Original lister. Receives the asset back and the listing rent refund.",
            "May be a PDA authority, so it must not be assumed system-owned."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "marketplaceConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  112,
                  108,
                  97,
                  99,
                  101,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplace_config.collection_mint",
                "account": "marketplaceConfig"
              }
            ]
          }
        },
        {
          "name": "listing",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  115,
                  116,
                  105,
                  110,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "asset"
              }
            ]
          }
        },
        {
          "name": "asset",
          "docs": [
            "re-check the asset key matches `listing.asset` via `has_one`."
          ],
          "writable": true,
          "relations": [
            "listing"
          ]
        },
        {
          "name": "collection",
          "writable": true
        },
        {
          "name": "escrow",
          "docs": [
            "back to seller via PDA seeds. No data, no init."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  115,
                  99,
                  114,
                  111,
                  119
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "asset"
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "initializeMarketplace",
      "docs": [
        "One-shot per collection. Initializes a `MarketplaceConfig` PDA, records",
        "the verified collection mint, and caches the mpl-core program id used",
        "by every subsequent listing/transfer."
      ],
      "discriminator": [
        47,
        81,
        64,
        0,
        96,
        56,
        105,
        7
      ],
      "accounts": [
        {
          "name": "payer",
          "docs": [
            "Pays rent for the new `MarketplaceConfig`."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "admin",
          "docs": [
            "sign so a stranger can't backdoor a config under someone else's admin key."
          ],
          "signer": true
        },
        {
          "name": "marketplaceConfig",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  112,
                  108,
                  97,
                  99,
                  101,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "collectionMint"
              }
            ]
          }
        },
        {
          "name": "collectionMint",
          "docs": [
            "mpl-core `CollectionV1` mint. Validated by Anchor via the `Owner` impl."
          ]
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "feeBps",
          "type": "u16"
        },
        {
          "name": "feeRecipient",
          "type": "pubkey"
        },
        {
          "name": "minPriceLamports",
          "type": "u64"
        },
        {
          "name": "mplCoreProgram",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "listNft",
      "docs": [
        "Escrow asset to `[b\"escrow\", config, asset]` PDA and create a `Listing`."
      ],
      "discriminator": [
        88,
        221,
        93,
        166,
        63,
        220,
        106,
        232
      ],
      "accounts": [
        {
          "name": "payer",
          "docs": [
            "Pays rent for the new `Listing` account and any mpl-core transfer",
            "reallocations. Usually the same wallet as `seller`; protocol-owned",
            "listings may use a separate system-owned payer PDA."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "seller",
          "docs": [
            "Current asset owner. Signs the escrow `TransferV1`. This may be a PDA",
            "authority, so do not assume it is system-owned or able to pay SOL."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "marketplaceConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  112,
                  108,
                  97,
                  99,
                  101,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplace_config.collection_mint",
                "account": "marketplaceConfig"
              }
            ]
          }
        },
        {
          "name": "listing",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  115,
                  116,
                  105,
                  110,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "asset"
              }
            ]
          }
        },
        {
          "name": "asset",
          "docs": [
            "Validated below by deserializing as `BaseAssetV1` and re-checking key."
          ],
          "writable": true
        },
        {
          "name": "collection",
          "docs": [
            "Must match `marketplace_config.collection_mint`. Mut because mpl-core",
            "updates `current_size` etc. on transfer for assets in a collection."
          ],
          "writable": true
        },
        {
          "name": "escrow",
          "docs": [
            "the asset's new `owner` field. We never `init` it; the address has no",
            "data account and zero lamports. mpl-core's TransferV1 only reads the",
            "pubkey of `new_owner`, so an AccountInfo with the right key is enough."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  115,
                  99,
                  114,
                  111,
                  119
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "asset"
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram"
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
      "name": "reclaimStaleListing",
      "docs": [
        "Permissionless reclaim of a stale listing whose asset owner no longer",
        "matches the recorded seller. Closes the listing and refunds rent to caller."
      ],
      "discriminator": [
        11,
        44,
        216,
        75,
        162,
        69,
        115,
        145
      ],
      "accounts": [
        {
          "name": "caller",
          "docs": [
            "Caller receives the listing rent refund as incentive."
          ],
          "writable": true,
          "signer": true
        },
        {
          "name": "marketplaceConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  112,
                  108,
                  97,
                  99,
                  101,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplace_config.collection_mint",
                "account": "marketplaceConfig"
              }
            ]
          }
        },
        {
          "name": "listing",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  115,
                  116,
                  105,
                  110,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "asset"
              }
            ]
          }
        },
        {
          "name": "asset",
          "docs": [
            "longer escrowed by this marketplace."
          ],
          "writable": true,
          "relations": [
            "listing"
          ]
        },
        {
          "name": "escrow",
          "docs": [
            "mpl-core asset owner set to this PDA. The listing is reclaimable only",
            "after the asset is no longer owned by escrow."
          ],
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  101,
                  115,
                  99,
                  114,
                  111,
                  119
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "asset"
              }
            ]
          }
        },
        {
          "name": "mplCoreProgram"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": []
    },
    {
      "name": "updateListingPrice",
      "docs": [
        "Seller re-prices an existing listing. Subject to `min_price_lamports`."
      ],
      "discriminator": [
        103,
        80,
        184,
        80,
        159,
        24,
        94,
        138
      ],
      "accounts": [
        {
          "name": "seller",
          "signer": true
        },
        {
          "name": "marketplaceConfig",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  97,
                  114,
                  107,
                  101,
                  116,
                  112,
                  108,
                  97,
                  99,
                  101,
                  45,
                  99,
                  111,
                  110,
                  102,
                  105,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplace_config.collection_mint",
                "account": "marketplaceConfig"
              }
            ]
          }
        },
        {
          "name": "listing",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  108,
                  105,
                  115,
                  116,
                  105,
                  110,
                  103
                ]
              },
              {
                "kind": "account",
                "path": "marketplaceConfig"
              },
              {
                "kind": "account",
                "path": "listing.asset",
                "account": "listing"
              }
            ]
          }
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
      "name": "updateMarketplaceConfig",
      "docs": [
        "Admin-only. Each `Some` field is overwritten; `None` leaves the field as-is."
      ],
      "discriminator": [
        255,
        146,
        152,
        33,
        80,
        216,
        160,
        144
      ],
      "accounts": [
        {
          "name": "admin",
          "signer": true,
          "relations": [
            "marketplaceConfig"
          ]
        },
        {
          "name": "marketplaceConfig",
          "writable": true
        }
      ],
      "args": [
        {
          "name": "feeBps",
          "type": {
            "option": "u16"
          }
        },
        {
          "name": "feeRecipient",
          "type": {
            "option": "pubkey"
          }
        },
        {
          "name": "minPriceLamports",
          "type": {
            "option": "u64"
          }
        },
        {
          "name": "enabled",
          "type": {
            "option": "bool"
          }
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "baseCollectionV1",
      "discriminator": [
        5
      ]
    },
    {
      "name": "listing",
      "discriminator": [
        218,
        32,
        50,
        73,
        43,
        134,
        26,
        58
      ]
    },
    {
      "name": "marketplaceConfig",
      "discriminator": [
        169,
        22,
        247,
        131,
        182,
        200,
        81,
        124
      ]
    }
  ],
  "events": [
    {
      "name": "listingCancelled",
      "discriminator": [
        11,
        46,
        163,
        10,
        103,
        80,
        139,
        194
      ]
    },
    {
      "name": "listingPriceUpdated",
      "discriminator": [
        85,
        181,
        185,
        147,
        101,
        54,
        37,
        147
      ]
    },
    {
      "name": "listingReclaimed",
      "discriminator": [
        28,
        100,
        167,
        216,
        87,
        141,
        176,
        49
      ]
    },
    {
      "name": "marketplaceConfigUpdated",
      "discriminator": [
        126,
        55,
        250,
        58,
        219,
        209,
        181,
        12
      ]
    },
    {
      "name": "marketplaceInitialized",
      "discriminator": [
        22,
        167,
        42,
        34,
        172,
        55,
        155,
        14
      ]
    },
    {
      "name": "nftListed",
      "discriminator": [
        115,
        235,
        107,
        89,
        89,
        231,
        135,
        26
      ]
    },
    {
      "name": "nftSold",
      "discriminator": [
        82,
        21,
        49,
        86,
        87,
        54,
        132,
        103
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "marketplaceDisabled",
      "msg": "Marketplace is disabled"
    },
    {
      "code": 6001,
      "name": "priceTooLow",
      "msg": "Price below minimum"
    },
    {
      "code": 6002,
      "name": "priceTooHigh",
      "msg": "Listing price exceeds buyer max price"
    },
    {
      "code": 6003,
      "name": "feeTooHigh",
      "msg": "Fee exceeds max 10%"
    },
    {
      "code": 6004,
      "name": "notCollectionMember",
      "msg": "Asset not in registered collection"
    },
    {
      "code": 6005,
      "name": "sellerMismatch",
      "msg": "Seller mismatch"
    },
    {
      "code": 6006,
      "name": "insufficientFunds",
      "msg": "Insufficient buyer funds"
    },
    {
      "code": 6007,
      "name": "invalidMplCoreProgram",
      "msg": "Invalid MPL Core program"
    },
    {
      "code": 6008,
      "name": "unauthorized",
      "msg": "Admin only"
    },
    {
      "code": 6009,
      "name": "unsupportedPlugin",
      "msg": "Asset has unsupported plugin"
    },
    {
      "code": 6010,
      "name": "mathOverflow",
      "msg": "Math overflow"
    },
    {
      "code": 6011,
      "name": "invalidFeeRecipient",
      "msg": "Invalid fee recipient"
    },
    {
      "code": 6012,
      "name": "invalidCollection",
      "msg": "Invalid collection"
    },
    {
      "code": 6013,
      "name": "invalidAsset",
      "msg": "Asset deserialization failed"
    },
    {
      "code": 6014,
      "name": "listingNotStale",
      "msg": "Listing is not stale — asset is still held by marketplace escrow"
    }
  ],
  "types": [
    {
      "name": "baseCollectionV1",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "key",
            "type": {
              "defined": {
                "name": "key"
              }
            }
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
          },
          {
            "name": "numMinted",
            "type": "u32"
          },
          {
            "name": "currentSize",
            "type": "u32"
          }
        ]
      }
    },
    {
      "name": "key",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "uninitialized"
          },
          {
            "name": "assetV1"
          },
          {
            "name": "hashedAssetV1"
          },
          {
            "name": "pluginHeaderV1"
          },
          {
            "name": "pluginRegistryV1"
          },
          {
            "name": "collectionV1"
          }
        ]
      }
    },
    {
      "name": "listing",
      "docs": [
        "PDA: `[b\"listing\", marketplace_config, asset]`",
        "",
        "Existence of this account ⇔ listing is active. Cancel and buy both close",
        "the account (rent refund flows to the seller in both cases)."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "seller",
            "type": "pubkey"
          },
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "priceLamports",
            "type": "u64"
          },
          {
            "name": "createdAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "listingCancelled",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "seller",
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
      "name": "listingPriceUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "seller",
            "type": "pubkey"
          },
          {
            "name": "newPriceLamports",
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
      "name": "listingReclaimed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "oldSeller",
            "type": "pubkey"
          },
          {
            "name": "newOwner",
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
      "name": "marketplaceConfig",
      "docs": [
        "PDA: `[b\"marketplace-config\", collection_mint]`",
        "",
        "Per-collection configuration for the marketplace. The cached",
        "`mpl_core_program` is checked on every transfer-bearing ix so a malicious",
        "caller can't slip in a fake mpl-core program account."
      ],
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "bump",
            "type": "u8"
          },
          {
            "name": "admin",
            "type": "pubkey"
          },
          {
            "name": "enabled",
            "type": "bool"
          },
          {
            "name": "collectionMint",
            "docs": [
              "Verified mpl-core `CollectionV1` mint."
            ],
            "type": "pubkey"
          },
          {
            "name": "feeBps",
            "docs": [
              "300 = 3.00%"
            ],
            "type": "u16"
          },
          {
            "name": "feeRecipient",
            "docs": [
              "SOL recipient for marketplace fees (mineBTC `fee_recipient`)."
            ],
            "type": "pubkey"
          },
          {
            "name": "minPriceLamports",
            "docs": [
              "Hard floor on listing prices in lamports."
            ],
            "type": "u64"
          },
          {
            "name": "mplCoreProgram",
            "docs": [
              "Cached MPL Core program id used in transfer CPIs."
            ],
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "marketplaceConfigUpdated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "config",
            "type": "pubkey"
          },
          {
            "name": "feeBps",
            "type": "u16"
          },
          {
            "name": "feeRecipient",
            "type": "pubkey"
          },
          {
            "name": "enabled",
            "type": "bool"
          },
          {
            "name": "minPriceLamports",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "marketplaceInitialized",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "config",
            "type": "pubkey"
          },
          {
            "name": "collectionMint",
            "type": "pubkey"
          },
          {
            "name": "feeBps",
            "type": "u16"
          }
        ]
      }
    },
    {
      "name": "nftListed",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "asset",
            "type": "pubkey"
          },
          {
            "name": "seller",
            "type": "pubkey"
          },
          {
            "name": "priceLamports",
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
      "name": "nftSold",
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
            "name": "priceLamports",
            "type": "u64"
          },
          {
            "name": "feeLamports",
            "type": "u64"
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
