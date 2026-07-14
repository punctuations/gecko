def restock(inventory, item, count):
    inventory[item] = inventory.get(item, 0) + count


stock = {"apples": 3, "pears": 7}
restock(stock, "apples", 5)
restock(stock, "plums", 2)

low = []
for item in stock:
    if stock[item] < 5:
        low.append(item)

print(stock)
print("low:", low)
print("kinds:", len(stock))
