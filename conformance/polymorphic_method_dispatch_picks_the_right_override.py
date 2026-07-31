class Animal:
    def speak(self):
        return "..."
class Dog(Animal):
    def speak(self):
        return "woof"
class Cat(Animal):
    def speak(self):
        return "meow"
def go(a):
    return a.speak()
d = Dog()
c = Cat()
print(go(d), go(c), go(d), go(c))
