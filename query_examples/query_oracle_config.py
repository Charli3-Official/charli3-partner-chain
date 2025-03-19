''' Utility script to query the Oracle configuration for the Charli3 Oracle
    Partner Chain.
    Made with the library py-substrate-interface.
    Reference & installation guide: https://github.com/polkascan/py-substrate-interface
'''

from substrateinterface import SubstrateInterface # type: ignore

substrate = SubstrateInterface(url="ws://82.25.79.166:9944")

storage_keys = [
    substrate.create_storage_key(
        "Oracle", "FeedAge"
    ),
    substrate.create_storage_key(
        "Oracle", "OutliersRange"
    )
]

result = substrate.query_multi(storage_keys)

for storage_key, value_obj in result:
    print(storage_key.storage_function, value_obj)
