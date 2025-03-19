''' Utility script to query the AggregationStatus for the Charli3 Oracle
    Partner Chain.
    Made with the library py-substrate-interface.
    Reference & installation guide: https://github.com/polkascan/py-substrate-interface
'''

from substrateinterface import SubstrateInterface # type: ignore
from substrateinterface.extensions import SubstrateNodeExtension # type: ignore
import json

def get_reward_candidates(event):
    status = event[0].value['attributes']['status']
    block = event[0].value['attributes']['block']

    if status == 'AggregationNotPerformed':
        print(f"Can't retrieve reward elegible nodes because aggregation wasn't performed for block {block}")
    else:
        rc = status['AggregationPerformed']['reward_elegible_nodes']
        print(f"Reward candidates for block {block} are:")
        for candidate in rc:
            print(candidate)

def get_aggregation_status(event):
    status = event[0].value['attributes']['status'] 
    block = event[0].value['attributes']['block']
    price = event[0].value['attributes']['median_price']

    print(f"Aggregated median price for block {block} is {price}")
    print(f"Aggregation status for block {block}:")
    print(status)
    
substrate = SubstrateInterface(url="ws://82.25.79.166:9944")

substrate.register_extension(SubstrateNodeExtension(max_block_range=1))

status_event = substrate.extensions.filter_events(pallet_name="Oracle", event_name="Status")

if status_event:
    get_aggregation_status(status_event)
    # get_reward_candidates(status_event)
else:
    print("No events for block")
