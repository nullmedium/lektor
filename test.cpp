// C++ test file for syntax highlighting
#include <iostream>
#include <vector>
#include <memory>
#include <algorithm>

// Template class example
template<typename T>
class Container {
private:
    std::vector<T> items;

public:
    Container() = default;

    void add(const T& item) {
        items.push_back(item);
    }

    T& operator[](size_t index) {
        return items[index];
    }

    size_t size() const noexcept {
        return items.size();
    }
};

// Namespace example
namespace Utils {
    inline int factorial(int n) {
        return (n <= 1) ? 1 : n * factorial(n - 1);
    }

    constexpr double PI = 3.14159265359;
}

// Modern C++ features
class Shape {
public:
    virtual ~Shape() = default;
    virtual double area() const = 0;
    virtual void draw() const = 0;
};

class Circle : public Shape {
private:
    double radius;

public:
    explicit Circle(double r) : radius(r) {}

    double area() const override {
        return Utils::PI * radius * radius;
    }

    void draw() const override {
        std::cout << "Drawing circle with radius: " << radius << std::endl;
    }
};

int main() {
    // Auto type deduction
    auto container = std::make_unique<Container<int>>();

    // Range-based for loop
    std::vector<int> numbers = {1, 2, 3, 4, 5};
    for (const auto& num : numbers) {
        container->add(num * 2);
    }

    // Lambda expression
    auto sum = [](const auto& vec) {
        int total = 0;
        for (const auto& val : vec) {
            total += val;
        }
        return total;
    };

    // Smart pointers
    std::unique_ptr<Shape> circle = std::make_unique<Circle>(5.0);
    std::cout << "Area: " << circle->area() << std::endl;

    // String literals
    std::string message = "Hello, C++20!";
    std::cout << message << std::endl;

    return 0;
}
