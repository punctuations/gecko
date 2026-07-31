class Animal:
    def __init__(self, name):
        self.name = name
    def speak(self):
        return "..."
    def describe(self):
        return self.name + ": " + self.speak()
class Dog(Animal):
    def speak(self):
        return "woof"
print(Dog("Rex").describe())
print(Animal("Thing").describe())
