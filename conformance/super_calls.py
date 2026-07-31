class Animal:
    def __init__(self, name):
        self.name = name
    def speak(self):
        return "..."
    def describe(self):
        return self.name + " says " + self.speak()

class Dog(Animal):
    def __init__(self, name, breed):
        super().__init__(name)
        self.breed = breed
    def speak(self):
        return "woof"
    def describe(self):
        return super().describe() + " (" + self.breed + ")"

class Puppy(Dog):
    def speak(self):
        return super().speak() + "!"

d = Dog("Rex", "lab")
print(d.name, d.breed, d.speak())
print(d.describe())
p = Puppy("Bit", "corgi")
print(p.speak(), p.describe())
print(super(Dog, d).speak())
print(isinstance(d, Dog), isinstance(d, Animal))
